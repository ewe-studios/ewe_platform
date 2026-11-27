//! The Durable Streams client.
//!
//! The client speaks the wire protocol over an injected transport. It owns no
//! socket and no runtime.

use super::dynamic::DynamicHeaders;
use super::retry::RetryPolicy;
use super::transport::{
    ClientRequest, ClientResponse, ClientTransport, HttpMethod, TransportError,
};
use crate::durable::{ContentType, StreamName};
use foundation_context::Ctx;
use std::collections::BTreeMap;
use std::sync::Arc;

/// The client configuration.
#[derive(Clone, Debug)]
pub struct ClientConfig {
    /// The server base URL, without a trailing slash.
    pub base_url: String,
    /// The headers every request carries.
    pub default_headers: Vec<(String, String)>,
}

/// The client behaviour that a stream handle inherits.
///
/// The values come from [`DurableClient`] and can be changed with its builder
/// methods. A clone is cheap, so every handle holds the set it was built with.
#[derive(Clone, Default)]
struct ClientSettings {
    /// When to repeat a request.
    retry: RetryPolicy,
    /// The injected clock and random number generator.
    ctx: Option<Ctx>,
    /// The provider of headers and query parameters for each request.
    dynamic_headers: Option<Arc<dyn DynamicHeaders>>,
}

/// A client failure.
#[derive(Debug)]
pub enum ClientError {
    /// The transport failed.
    Transport(String),
    /// The stream is absent.
    NotFound {
        /// The requested path.
        path: String,
    },
    /// The stream is soft-deleted.
    Gone {
        /// The requested path.
        path: String,
    },
    /// The request conflicts with the stream state.
    Conflict(String),
    /// The stream is closed.
    Closed,
    /// An offset is malformed.
    InvalidOffset(String),
    /// The server answered an unexpected status.
    UnexpectedStatus {
        /// The status code.
        status: u16,
        /// A bounded message body.
        body: String,
        /// The response headers.
        headers: BTreeMap<String, String>,
    },
    /// The server body is malformed.
    Parse(String),
    /// The method is not available on this surface.
    NotSupported(String),
    /// The client used every attempt and the failure stayed transient.
    RetryExhausted {
        /// The number of attempts that ran, including the first one.
        attempts: u32,
        /// The last status, when the failure was a status.
        status: Option<u16>,
        /// A bounded message body or the transport message.
        body: String,
        /// The response headers of the last attempt.
        headers: BTreeMap<String, String>,
    },
    /// A configuration value is not usable.
    InvalidArgument {
        /// The field name.
        field: String,
        /// What is wrong with the value.
        message: String,
    },
    /// A retry needs a timer, but the [`Ctx`] has none.
    MissingClock(String),
    /// The producer was detached.
    Detached,
}

impl core::fmt::Display for ClientError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Transport(message) => write!(f, "network error: {message}"),
            Self::NotFound { path } => write!(f, "stream not found: {path}"),
            Self::Gone { path } => write!(f, "stream is gone: {path}"),
            Self::Conflict(message) => write!(f, "conflict: {message}"),
            Self::Closed => f.write_str("stream is closed"),
            Self::InvalidOffset(value) => write!(f, "invalid offset: {value}"),
            Self::UnexpectedStatus { status, body, .. } => {
                write!(f, "unexpected status {status}: {body}")
            }
            Self::Parse(message) => write!(f, "parse error: {message}"),
            Self::NotSupported(message) => write!(f, "not supported: {message}"),
            Self::RetryExhausted {
                attempts,
                status,
                body,
                ..
            } => match status {
                Some(status) => write!(
                    f,
                    "retry exhausted after {attempts} attempts: unexpected status {status}: {body}"
                ),
                None => write!(f, "retry exhausted after {attempts} attempts: {body}"),
            },
            Self::InvalidArgument { field, message } => {
                write!(f, "invalid argument: {field}: {message}")
            }
            Self::MissingClock(message) => write!(f, "missing clock: {message}"),
            Self::Detached => f.write_str("the producer is detached"),
        }
    }
}

impl core::error::Error for ClientError {}

impl From<TransportError> for ClientError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error.message().to_string())
    }
}

/// The live read mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadMode {
    /// Read what exists now.
    CatchUp,
    /// Wait for new data, then answer.
    LongPoll,
    /// Keep the connection open and send events.
    Sse,
    /// Catch up, then choose a live mode.
    Auto,
}

/// One chunk of stream data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadChunk {
    /// The bytes of the message.
    pub data: Vec<u8>,
}

/// The result of a read.
#[derive(Clone, Debug, Default)]
pub struct ReadResult {
    /// The HTTP status.
    pub status: u16,
    /// The chunks read.
    pub chunks: Vec<ReadChunk>,
    /// The next wire offset.
    pub next_offset: Option<String>,
    /// True when the client caught up with the tail.
    pub up_to_date: bool,
    /// True when the stream is closed.
    pub closed: bool,
    /// The collapsing cursor.
    pub cursor: Option<String>,
}

/// The result of a head request.
#[derive(Clone, Debug, Default)]
pub struct HeadResult {
    /// The HTTP status.
    pub status: u16,
    /// The tail wire offset.
    pub offset: Option<String>,
    /// The content type.
    pub content_type: Option<String>,
    /// The time-to-live seconds.
    pub ttl_seconds: Option<u64>,
    /// The absolute deadline in RFC 3339 form.
    pub expires_at: Option<String>,
    /// True when the stream is closed.
    pub closed: bool,
}

/// The result of an append, create or close.
#[derive(Clone, Debug, Default)]
pub struct AppendResult {
    /// The HTTP status.
    pub status: u16,
    /// The next wire offset.
    pub offset: Option<String>,
    /// The response headers of interest.
    pub headers: BTreeMap<String, String>,
    /// True when the server treated the write as a duplicate.
    pub duplicate: bool,
}

/// A Durable Streams client over an injected transport.
#[derive(Clone)]
pub struct DurableClient {
    transport: Arc<dyn ClientTransport>,
    config: ClientConfig,
    settings: ClientSettings,
}

impl DurableClient {
    /// Build a client with the default retry policy and no clock.
    ///
    /// A client with no clock does not retry: a retry wait needs an injected
    /// clock. Call [`Self::with_ctx`] to enable retry.
    #[must_use]
    pub fn new(transport: Arc<dyn ClientTransport>, config: ClientConfig) -> Self {
        Self {
            transport,
            config,
            settings: ClientSettings::default(),
        }
    }

    /// Use this retry policy for every request.
    #[must_use]
    pub fn with_retry_policy(mut self, retry: RetryPolicy) -> Self {
        self.settings.retry = retry;
        self
    }

    /// Use this [`Ctx`] for the retry clock and the jitter source.
    #[must_use]
    pub fn with_ctx(mut self, ctx: Ctx) -> Self {
        self.settings.ctx = Some(ctx);
        self
    }

    /// Evaluate this provider for every request the client sends.
    #[must_use]
    pub fn with_dynamic_headers(mut self, provider: Arc<dyn DynamicHeaders>) -> Self {
        self.settings.dynamic_headers = Some(provider);
        self
    }

    /// Return a handle for a stream name.
    #[must_use]
    pub fn stream(&self, name: &StreamName) -> StreamHandle {
        self.handle(&format!("/{}", name.as_str()))
    }

    /// Return a handle for a path relative to the base URL.
    #[must_use]
    pub fn handle(&self, path: &str) -> StreamHandle {
        let base = self.config.base_url.trim_end_matches('/');
        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        StreamHandle {
            transport: self.transport.clone(),
            url: format!("{base}{path}"),
            path,
            headers: self.config.default_headers.clone(),
            extra_query: Vec::new(),
            settings: self.settings.clone(),
        }
    }
}

/// A handle for one stream.
#[derive(Clone)]
pub struct StreamHandle {
    transport: Arc<dyn ClientTransport>,
    url: String,
    path: String,
    headers: Vec<(String, String)>,
    extra_query: Vec<(String, String)>,
    settings: ClientSettings,
}

impl StreamHandle {
    /// The stream path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The retry policy of this handle.
    #[must_use]
    pub fn retry_policy(&self) -> &RetryPolicy {
        &self.settings.retry
    }

    /// The clock and random source of this handle, when it has one.
    #[must_use]
    pub fn ctx(&self) -> Option<&Ctx> {
        self.settings.ctx.as_ref()
    }

    /// Return a copy of this handle that adds these headers to every request.
    #[must_use]
    pub fn with_headers(&self, headers: Vec<(String, String)>) -> Self {
        let mut clone = self.clone();
        clone.headers.extend(headers);
        clone
    }

    /// Return a copy of this handle that adds query parameters to every request.
    #[must_use]
    pub fn with_query(&self, params: Vec<(String, String)>) -> Self {
        let mut clone = self.clone();
        clone.extra_query.extend(params);
        clone
    }

    /// Return a copy of this handle that evaluates this provider for every
    /// request.
    #[must_use]
    pub fn with_dynamic_headers(&self, provider: Arc<dyn DynamicHeaders>) -> Self {
        let mut clone = self.clone();
        clone.settings.dynamic_headers = Some(provider);
        clone
    }

    /// Run `attempt` under this handle's retry policy.
    ///
    /// The closure receives the one-based attempt number. It runs again after a
    /// transient failure, so it can rebuild the request and evaluate dynamic
    /// headers again.
    ///
    /// # Errors
    ///
    /// Returns the permanent failure, [`ClientError::RetryExhausted`] after the
    /// last attempt, or [`ClientError::MissingClock`] when the handle has a
    /// [`Ctx`] with no timer. A handle with no [`Ctx`] does not retry.
    pub async fn with_retry<T, F, Fut>(&self, attempt: F) -> Result<T, ClientError>
    where
        F: FnMut(u32) -> Fut,
        Fut: core::future::Future<Output = Result<T, ClientError>>,
    {
        self.settings
            .retry
            .execute(self.settings.ctx.as_ref(), attempt)
            .await
    }

    fn request(
        &self,
        method: HttpMethod,
        query: &str,
        headers: &[(String, String)],
        body: Vec<u8>,
    ) -> ClientRequest {
        let mut all = self.headers.clone();
        if let Some(provider) = &self.settings.dynamic_headers {
            all.extend(provider.headers());
        }
        all.extend_from_slice(headers);
        let mut query = query.to_string();
        append_query(&mut query, &self.extra_query);
        if let Some(provider) = &self.settings.dynamic_headers {
            append_query(&mut query, &provider.query());
        }
        let url = if query.is_empty() {
            self.url.clone()
        } else {
            format!("{}?{query}", self.url)
        };
        ClientRequest {
            method,
            url,
            headers: all,
            body,
        }
    }

    /// Create the stream with an explicit wire configuration.
    ///
    /// # Errors
    /// Returns [`ClientError`] for a failed transport or a status.
    pub async fn create(
        &self,
        content_type: Option<&str>,
        ttl_seconds: Option<u64>,
        expires_at: Option<&str>,
        closed: bool,
        data: Vec<u8>,
    ) -> Result<AppendResult, ClientError> {
        let mut headers = Vec::new();
        if let Some(content_type) = content_type {
            headers.push(("content-type".to_string(), content_type.to_string()));
        }
        if let Some(ttl) = ttl_seconds {
            headers.push(("stream-ttl".to_string(), ttl.to_string()));
        }
        if let Some(expires_at) = expires_at {
            headers.push(("stream-expires-at".to_string(), expires_at.to_string()));
        }
        if closed {
            headers.push(("stream-closed".to_string(), "true".to_string()));
        }
        self.with_retry(|_attempt| {
            let handle = self.clone();
            let headers = headers.clone();
            let body = data.clone();
            async move {
                let response = handle
                    .transport
                    .send(handle.request(HttpMethod::Put, "", &headers, body))
                    .await
                    .map_err(ClientError::from)?;
                outcome(&handle.path, &response, false)
            }
        })
        .await
    }

    /// Append a message.
    ///
    /// A plain append does not retry: a repeated write could duplicate data.
    /// An append with a producer retries, because the producer sends the same
    /// sequence and the server deduplicates it.
    ///
    /// # Errors
    /// Returns [`ClientError`] for a failed transport or a refused append.
    pub async fn append(
        &self,
        data: Vec<u8>,
        content_type: Option<&str>,
        stream_seq: Option<&str>,
        producer: Option<(&str, u64, u64)>,
        close: bool,
    ) -> Result<AppendResult, ClientError> {
        let mut headers = Vec::new();
        if let Some(content_type) = content_type {
            headers.push(("content-type".to_string(), content_type.to_string()));
        }
        if let Some(seq) = stream_seq {
            headers.push(("stream-seq".to_string(), seq.to_string()));
        }
        if let Some((id, epoch, seq)) = producer {
            headers.push(("producer-id".to_string(), id.to_string()));
            headers.push(("producer-epoch".to_string(), epoch.to_string()));
            headers.push(("producer-seq".to_string(), seq.to_string()));
        }
        if close {
            headers.push(("stream-closed".to_string(), "true".to_string()));
        }
        let producer_write = producer.is_some();
        let send = |_attempt: u32| {
            let handle = self.clone();
            let headers = headers.clone();
            let body = data.clone();
            async move {
                let response = handle
                    .transport
                    .send(handle.request(HttpMethod::Post, "", &headers, body))
                    .await
                    .map_err(ClientError::from)?;
                outcome(&handle.path, &response, producer_write)
            }
        };
        if producer_write {
            self.with_retry(send).await
        } else {
            send(1).await
        }
    }

    /// Read stream metadata.
    ///
    /// # Errors
    /// Returns [`ClientError`] for a failed transport or a status.
    pub async fn head(&self) -> Result<HeadResult, ClientError> {
        self.with_retry(|_attempt| {
            let handle = self.clone();
            async move {
                let response = handle
                    .transport
                    .send(handle.request(HttpMethod::Head, "", &[], Vec::new()))
                    .await
                    .map_err(ClientError::from)?;
                head_outcome(&handle.path, &response)
            }
        })
        .await
    }

    /// Delete the stream.
    ///
    /// # Errors
    /// Returns [`ClientError`] for a failed transport or a status.
    pub async fn delete(&self) -> Result<u16, ClientError> {
        self.with_retry(|_attempt| {
            let handle = self.clone();
            async move {
                let response = handle
                    .transport
                    .send(handle.request(HttpMethod::Delete, "", &[], Vec::new()))
                    .await
                    .map_err(ClientError::from)?;
                match response.status {
                    200 | 204 => Ok(response.status),
                    404 => Err(ClientError::NotFound {
                        path: handle.path.clone(),
                    }),
                    410 => Err(ClientError::Gone {
                        path: handle.path.clone(),
                    }),
                    status => Err(unexpected(status, &response)),
                }
            }
        })
        .await
    }

    /// Close the stream, optionally with a final message.
    ///
    /// A plain close does not retry, for the same reason as a plain append. A
    /// producer closes through [`super::producer::Producer::close`], which is
    /// safe to retry.
    ///
    /// # Errors
    /// Returns [`ClientError`] for a failed transport or a status.
    pub async fn close(
        &self,
        data: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<AppendResult, ClientError> {
        let mut headers = vec![("stream-closed".to_string(), "true".to_string())];
        if !data.is_empty() {
            if let Some(content_type) = content_type {
                headers.push(("content-type".to_string(), content_type.to_string()));
            }
        }
        let response = self
            .transport
            .send(self.request(HttpMethod::Post, "", &headers, data))
            .await?;
        outcome(&self.path, &response, false)
    }

    /// Start a read.
    #[must_use]
    pub fn read(&self) -> ReadBuilder {
        ReadBuilder {
            handle: self.clone(),
            offset: None,
            mode: ReadMode::CatchUp,
            headers: Vec::new(),
            max_chunks: None,
            sse_binary: false,
            cursor: None,
        }
    }
}

/// Builds one read.
#[derive(Clone)]
pub struct ReadBuilder {
    handle: StreamHandle,
    offset: Option<String>,
    mode: ReadMode,
    headers: Vec<(String, String)>,
    max_chunks: Option<usize>,
    sse_binary: bool,
    cursor: Option<String>,
}

impl ReadBuilder {
    /// Set the starting wire offset. `None` starts at the beginning.
    #[must_use]
    pub fn offset(mut self, offset: impl Into<String>) -> Self {
        self.offset = Some(offset.into());
        self
    }

    /// Set the live mode.
    #[must_use]
    pub fn mode(mut self, mode: ReadMode) -> Self {
        self.mode = mode;
        self
    }

    /// Add a request header.
    #[must_use]
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Stop after this many chunks.
    #[must_use]
    pub fn max_chunks(mut self, count: usize) -> Self {
        self.max_chunks = Some(count);
        self
    }

    /// Tell the reader that a Server-Sent Events (SSE) payload is base64.
    #[must_use]
    pub fn sse_binary(mut self, binary: bool) -> Self {
        self.sse_binary = binary;
        self
    }

    /// Send a conditional read with this entity tag.
    #[must_use]
    pub fn if_none_match(mut self, tag: impl Into<String>) -> Self {
        self.headers.push(("if-none-match".to_string(), tag.into()));
        self
    }

    /// Echo a collapsing cursor from a previous response.
    #[must_use]
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Build the wire request for this read.
    fn request(&self) -> ClientRequest {
        let mut query = String::new();
        if let Some(offset) = &self.offset {
            query.push_str("offset=");
            query.push_str(offset);
        } else {
            // An omitted offset reads from the start of the stream.
            query.push_str("offset=-1");
        }
        if let Some(cursor) = &self.cursor {
            if !query.is_empty() {
                query.push('&');
            }
            query.push_str("cursor=");
            query.push_str(cursor);
        }
        match self.mode {
            ReadMode::CatchUp | ReadMode::Auto => {}
            ReadMode::LongPoll => {
                if !query.is_empty() {
                    query.push('&');
                }
                query.push_str("live=long-poll");
            }
            ReadMode::Sse => {
                if !query.is_empty() {
                    query.push('&');
                }
                query.push_str("live=sse");
            }
        }
        self.handle
            .request(HttpMethod::Get, &query, &self.headers, Vec::new())
    }

    /// Send the read.
    ///
    /// Handles catch-up, long-poll and auto. Use [`Self::open_sse`] for
    /// [`ReadMode::Sse`], whose live connection yields a stream of events.
    ///
    /// # Errors
    /// Returns [`ClientError`] for a failed transport or a status.
    pub async fn send(self) -> Result<ReadResult, ClientError> {
        if self.mode == ReadMode::Sse {
            return Err(ClientError::NotSupported(
                "use ReadBuilder::open_sse for Server-Sent Events".to_string(),
            ));
        }
        self.handle
            .with_retry(|_attempt| {
                let builder = self.clone();
                async move { builder.send_once().await }
            })
            .await
    }

    /// Send one read attempt.
    async fn send_once(&self) -> Result<ReadResult, ClientError> {
        let request = self.request();
        let response = self.handle.transport.send(request).await?;

        match response.status {
            304 => Ok(ReadResult {
                status: 304,
                next_offset: response.header("stream-next-offset").map(str::to_string),
                up_to_date: true,
                closed: response.header("stream-closed") == Some("true"),
                ..ReadResult::default()
            }),
            200 | 204 => {
                let closed = response.header("stream-closed") == Some("true");
                let up_to_date = response.header("stream-up-to-date") == Some("true");
                let cursor = response.header("stream-cursor").map(str::to_string);
                let next_offset = response.header("stream-next-offset").map(str::to_string);
                let mut chunks = parse_body(&response)?;
                if let Some(limit) = self.max_chunks {
                    chunks.truncate(limit);
                }
                Ok(ReadResult {
                    status: response.status,
                    chunks,
                    next_offset,
                    up_to_date,
                    closed,
                    cursor,
                })
            }
            404 => Err(ClientError::NotFound {
                path: self.handle.path.clone(),
            }),
            410 => Err(ClientError::Gone {
                path: self.handle.path.clone(),
            }),
            400 => Err(ClientError::InvalidOffset(
                self.offset.clone().unwrap_or_default(),
            )),
            status => Err(unexpected(status, &response)),
        }
    }
}

impl ReadBuilder {
    /// Open a live Server-Sent Events (SSE) read.
    ///
    /// The response head carries the status and headers. A `404` or `410`
    /// answers here before any event. The `stream-sse-data-encoding` header
    /// states whether data events are base64.
    ///
    /// # Errors
    /// Returns [`ClientError`] for a failed transport or a status.
    pub async fn open_sse(self) -> Result<SseRead, ClientError> {
        let request = self.request();
        let open = self.handle.transport.open_sse(request).await?;
        match open.status {
            200 | 204 => {}
            404 => {
                return Err(ClientError::NotFound {
                    path: self.handle.path.clone(),
                })
            }
            410 => {
                return Err(ClientError::Gone {
                    path: self.handle.path.clone(),
                })
            }
            status => return Err(unexpected_open(status, &open)),
        }
        let decode_base64 = open
            .header("stream-sse-data-encoding")
            .is_some_and(|value| value.eq_ignore_ascii_case("base64"))
            || self.sse_binary;
        Ok(SseRead {
            status: open.status,
            headers: open.headers,
            inner: open.stream,
            decode_base64,
            next_offset: None,
            cursor: None,
            up_to_date: false,
            closed: false,
        })
    }
}

/// A live Server-Sent Events (SSE) read.
///
/// The stream yields one decoded chunk per data event. A control event updates
/// the read metadata and yields an empty chunk, which ends the current step.
/// `None` ends the stream.
pub struct SseRead {
    status: u16,
    headers: BTreeMap<String, Vec<String>>,
    inner: super::transport::ClientSseStream,
    decode_base64: bool,
    next_offset: Option<String>,
    cursor: Option<String>,
    up_to_date: bool,
    closed: bool,
}

impl SseRead {
    /// The response status.
    #[must_use]
    pub fn status(&self) -> u16 {
        self.status
    }

    /// The first value of a response header, matched without case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .and_then(|(_, values)| values.first())
            .map(String::as_str)
    }

    /// The next wire offset known from the last control event.
    #[must_use]
    pub fn next_offset(&self) -> Option<&str> {
        self.next_offset.as_deref()
    }

    /// The collapsing cursor known from the last control event.
    #[must_use]
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    /// True when the last control event marked the read up to date.
    #[must_use]
    pub fn up_to_date(&self) -> bool {
        self.up_to_date
    }

    /// True when the last control event marked the stream closed.
    #[must_use]
    pub fn closed(&self) -> bool {
        self.closed
    }

    /// The next step of the read.
    ///
    /// A data event waits for the following control event and then yields its
    /// decoded bytes with that control's metadata. A control-only event yields
    /// an empty chunk with its metadata. `None` ends the stream.
    ///
    /// # Errors
    /// Returns [`ClientError`] for a transport failure, a malformed control
    /// event or a malformed base64 payload.
    pub async fn next(&mut self) -> Result<Option<ReadChunk>, ClientError> {
        loop {
            let Some(event) = next_event(&mut self.inner).await else {
                return Ok(None);
            };
            let event = event?;
            match event.event_type.as_deref() {
                Some("data") => {
                    // One data event is followed by one control event. Wait for
                    // it so the chunk carries the step metadata (offset, cursor,
                    // closed). Consecutive data events are concatenated.
                    let mut parts = vec![decode(&event.data, self.decode_base64)?];
                    loop {
                        let Some(following) = next_event(&mut self.inner).await else {
                            return Ok(Some(ReadChunk { data: join(parts) }));
                        };
                        let following = following?;
                        match following.event_type.as_deref() {
                            Some("data") => {
                                parts.push(decode(&following.data, self.decode_base64)?);
                            }
                            Some("control") | None => {
                                self.apply_control(&following.data)?;
                                return Ok(Some(ReadChunk { data: join(parts) }));
                            }
                            _ => {}
                        }
                    }
                }
                Some("control") | None => {
                    // A control-only event carries the step metadata without a
                    // data payload.
                    self.apply_control(&event.data)?;
                    return Ok(Some(ReadChunk { data: Vec::new() }));
                }
                _ => {}
            }
        }
    }

    /// Apply one control event's fields to the read metadata.
    fn apply_control(&mut self, data: &str) -> Result<(), ClientError> {
        // An empty or malformed control body is a parse error.
        let value = serde_json::from_str::<serde_json::Value>(data)
            .map_err(|error| ClientError::Parse(error.to_string()))?;
        self.next_offset = value["streamNextOffset"].as_str().map(str::to_string);
        // PROTOCOL.md section 5.8 gives the cursor as a string. A number is
        // tolerated for older peers that sent one.
        self.cursor = value["streamCursor"]
            .as_str()
            .map(str::to_string)
            .or_else(|| {
                value["streamCursor"]
                    .as_u64()
                    .map(|value| value.to_string())
            });
        self.up_to_date = value["upToDate"].as_bool().unwrap_or(false);
        self.closed = value["streamClosed"].as_bool().unwrap_or(false);
        Ok(())
    }
}

/// Decode one data event into bytes.
fn decode(data: &str, base64: bool) -> Result<Vec<u8>, ClientError> {
    if base64 {
        decode_base64(data)
    } else {
        Ok(data.to_string().into_bytes())
    }
}

/// Join buffered data parts into one chunk.
fn join(parts: Vec<Vec<u8>>) -> Vec<u8> {
    parts.into_iter().flatten().collect()
}

/// Decode one Server-Sent Events (SSE) base64 payload.
///
/// A server may split one payload across `data:` lines, so the parser joins
/// them with a newline. Remove those newlines before decoding (protocol
/// section 5.8).
fn decode_base64(data: &str) -> Result<Vec<u8>, ClientError> {
    let cleaned: String = data
        .chars()
        .filter(|character| *character != '\n' && *character != '\r')
        .collect();
    if cleaned.is_empty() {
        return Ok(Vec::new());
    }
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &cleaned)
        .map_err(|error| ClientError::Parse(error.to_string()))
}

/// Read the next Server-Sent Events (SSE) event.
#[cfg(feature = "std")]
async fn next_event(
    stream: &mut super::transport::ClientSseStream,
) -> Option<Result<super::transport::SseEvent, TransportError>> {
    futures_lite::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await
}

/// Read the next Server-Sent Events (SSE) event on a target without `std`.
#[cfg(not(feature = "std"))]
async fn next_event(
    stream: &mut super::transport::ClientSseStream,
) -> Option<Result<super::transport::SseEvent, TransportError>> {
    futures_lite::StreamExt::next(stream).await
}

fn unexpected_open(status: u16, open: &super::transport::SseOpen) -> ClientError {
    let mut headers = BTreeMap::new();
    for (key, values) in &open.headers {
        if let Some(value) = values.first() {
            headers.insert(key.clone(), value.clone());
        }
    }
    ClientError::UnexpectedStatus {
        status,
        body: String::new(),
        headers,
    }
}

/// Build the result of a head request.
fn head_outcome(path: &str, response: &ClientResponse) -> Result<HeadResult, ClientError> {
    match response.status {
        200 => Ok(HeadResult {
            status: 200,
            offset: response.header("stream-next-offset").map(str::to_string),
            content_type: response.header("content-type").map(str::to_string),
            ttl_seconds: response
                .header("stream-ttl")
                .and_then(|value| value.parse().ok()),
            expires_at: response.header("stream-expires-at").map(str::to_string),
            closed: response.header("stream-closed") == Some("true"),
        }),
        404 => Err(ClientError::NotFound {
            path: path.to_string(),
        }),
        410 => Err(ClientError::Gone {
            path: path.to_string(),
        }),
        status => Err(unexpected(status, response)),
    }
}

/// Append query parameters to a query string.
fn append_query(query: &mut String, params: &[(String, String)]) {
    for (name, value) in params {
        if !query.is_empty() {
            query.push('&');
        }
        query.push_str(name);
        query.push('=');
        query.push_str(value);
    }
}

/// Build the result of a create, append or close.
///
/// `duplicate_on_204` is true only when the write carried a producer. A plain
/// append answers `204` too, so its status cannot report a duplicate.
fn outcome(
    path: &str,
    response: &ClientResponse,
    duplicate_on_204: bool,
) -> Result<AppendResult, ClientError> {
    let mut headers = BTreeMap::new();
    for (key, values) in &response.headers {
        if let Some(value) = values.first() {
            headers.insert(key.clone(), value.clone());
        }
    }
    let result = AppendResult {
        status: response.status,
        offset: response.header("stream-next-offset").map(str::to_string),
        headers,
        duplicate: duplicate_on_204 && response.status == 204,
    };
    match response.status {
        200 | 201 | 204 => Ok(result),
        404 => Err(ClientError::NotFound {
            path: path.to_string(),
        }),
        410 => Err(ClientError::Gone {
            path: path.to_string(),
        }),
        409 => {
            if response.header("stream-closed") == Some("true") {
                Err(ClientError::Closed)
            } else {
                Err(ClientError::Conflict(bounded_body(response)))
            }
        }
        status => Err(unexpected(status, response)),
    }
}

fn parse_body(response: &ClientResponse) -> Result<Vec<ReadChunk>, ClientError> {
    if response.body.is_empty() {
        return Ok(Vec::new());
    }
    let content_type = response.header("content-type").unwrap_or_default();
    let normalized = ContentType::parse(content_type).unwrap_or_default();
    if normalized.is_json() {
        // A JavaScript Object Notation (JSON) response is one array body.
        let _: serde_json::Value = serde_json::from_slice(&response.body)
            .map_err(|error| ClientError::Parse(error.to_string()))?;
        Ok(vec![ReadChunk {
            data: response.body.clone(),
        }])
    } else {
        Ok(vec![ReadChunk {
            data: response.body.clone(),
        }])
    }
}

fn unexpected(status: u16, response: &ClientResponse) -> ClientError {
    let mut headers = BTreeMap::new();
    for (key, values) in &response.headers {
        if let Some(value) = values.first() {
            headers.insert(key.clone(), value.clone());
        }
    }
    ClientError::UnexpectedStatus {
        status,
        body: bounded_body(response),
        headers,
    }
}

fn bounded_body(response: &ClientResponse) -> String {
    const LIMIT: usize = 512;
    let text = String::from_utf8_lossy(&response.body);
    text.chars().take(LIMIT).collect()
}
