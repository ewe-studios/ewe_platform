//! The Durable Streams client.
//!
//! The client speaks the wire protocol over an injected transport. It owns no
//! socket and no runtime.

use crate::shared::durable::transport::{
    ClientRequest, ClientResponse, ClientTransport, HttpMethod, TransportError,
};
use foundation_core::durable::{ContentType, StreamName};
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
}

impl DurableClient {
    /// Build a client.
    #[must_use]
    pub fn new(transport: Arc<dyn ClientTransport>, config: ClientConfig) -> Self {
        Self { transport, config }
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
}

impl StreamHandle {
    /// The stream path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
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

    fn request(
        &self,
        method: HttpMethod,
        query: &str,
        headers: &[(String, String)],
        body: Vec<u8>,
    ) -> ClientRequest {
        let mut all = self.headers.clone();
        all.extend_from_slice(headers);
        let mut query = query.to_string();
        for (name, value) in &self.extra_query {
            if !query.is_empty() {
                query.push('&');
            }
            query.push_str(name);
            query.push('=');
            query.push_str(value);
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
        let response = self
            .transport
            .send(self.request(HttpMethod::Put, "", &headers, data))
            .await?;
        outcome(&self.path, response)
    }

    /// Append a message.
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
        let response = self
            .transport
            .send(self.request(HttpMethod::Post, "", &headers, data))
            .await?;
        outcome(&self.path, response)
    }

    /// Read stream metadata.
    ///
    /// # Errors
    /// Returns [`ClientError`] for a failed transport or a status.
    pub async fn head(&self) -> Result<HeadResult, ClientError> {
        let response = self
            .transport
            .send(self.request(HttpMethod::Head, "", &[], Vec::new()))
            .await?;
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
                path: self.path.clone(),
            }),
            410 => Err(ClientError::Gone {
                path: self.path.clone(),
            }),
            status => Err(unexpected(status, &response)),
        }
    }

    /// Delete the stream.
    ///
    /// # Errors
    /// Returns [`ClientError`] for a failed transport or a status.
    pub async fn delete(&self) -> Result<u16, ClientError> {
        let response = self
            .transport
            .send(self.request(HttpMethod::Delete, "", &[], Vec::new()))
            .await?;
        match response.status {
            200 | 204 => Ok(response.status),
            404 => Err(ClientError::NotFound {
                path: self.path.clone(),
            }),
            410 => Err(ClientError::Gone {
                path: self.path.clone(),
            }),
            status => Err(unexpected(status, &response)),
        }
    }

    /// Close the stream, optionally with a final message.
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
        outcome(&self.path, response)
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
            sse_timeout_ms: 2_000,
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
    sse_timeout_ms: u64,
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

    /// Echo a collapsing cursor from a previous response.
    #[must_use]
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Stop a Server-Sent Events (SSE) read after this many milliseconds.
    #[must_use]
    pub fn sse_timeout(mut self, milliseconds: u64) -> Self {
        self.sse_timeout_ms = milliseconds;
        self
    }

    /// Send the read.
    ///
    /// # Errors
    /// Returns [`ClientError`] for a failed transport or a status.
    pub async fn send(self) -> Result<ReadResult, ClientError> {
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
            ReadMode::CatchUp => {}
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
            ReadMode::Auto => {}
        }

        let request = self
            .handle
            .request(HttpMethod::Get, &query, &self.headers, Vec::new());
        if self.mode == ReadMode::Sse {
            return self.send_sse(request).await;
        }
        let response = self.handle.transport.send(request).await?;

        match response.status {
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
                self.offset.unwrap_or_default(),
            )),
            status => Err(unexpected(status, &response)),
        }
    }
}

impl ReadBuilder {
    async fn send_sse(self, request: ClientRequest) -> Result<ReadResult, ClientError> {
        #[cfg(not(feature = "std"))]
        use futures_lite::StreamExt;

        // The event stream carries no status, so one HEAD request decides
        // whether the stream exists and is readable.
        let probe = self
            .handle
            .transport
            .send(self.handle.request(HttpMethod::Head, "", &self.headers, Vec::new()))
            .await?;
        match probe.status {
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
            status => return Err(unexpected(status, &probe)),
        }

        // The head states the data encoding. Fall back to the caller's flag
        // when the transport cannot supply the header.
        let decode_base64 = probe
            .header("stream-sse-data-encoding")
            .is_some_and(|value| value.eq_ignore_ascii_case("base64"))
            || self.sse_binary;

        let mut stream = self.handle.transport.open_sse(request).await?;
        let mut result = ReadResult {
            status: 200,
            ..ReadResult::default()
        };
        #[cfg(feature = "std")]
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_millis(self.sse_timeout_ms);
        loop {
            #[cfg(feature = "std")]
            let item = next_sse(&mut stream, deadline).await;
            #[cfg(not(feature = "std"))]
            let item = stream.next().await;
            let Some(item) = item else { break };
            let event = item?;
            match event.event_type.as_deref() {
                Some("data") => {
                    // An empty data event carries no message.
                    if event.data.is_empty() {
                        continue;
                    }
                    let data = if decode_base64 {
                        base64::Engine::decode(
                            &base64::engine::general_purpose::STANDARD,
                            event.data.as_bytes(),
                        )
                        .map_err(|error| ClientError::Parse(error.to_string()))?
                    } else {
                        event.data.into_bytes()
                    };
                    result.chunks.push(ReadChunk { data });
                }
                Some("control") | None => {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&event.data) {
                        result.next_offset =
                            value["streamNextOffset"].as_str().map(str::to_string);
                        result.cursor = value["streamCursor"]
                            .as_u64()
                            .map(|value| value.to_string());
                        result.up_to_date =
                            value["upToDate"].as_bool().unwrap_or(false);
                        result.closed =
                            value["streamClosed"].as_bool().unwrap_or(false);
                    }
                    break;
                }
                _ => {}
            }
        }
        Ok(result)
    }
}

/// Read the next SSE event, or stop at the deadline.
///
/// The stream has no timer of its own, so the future wakes itself and checks
/// the wall clock. The wait is short and a test adapter can afford it.
#[cfg(feature = "std")]
async fn next_sse(
    stream: &mut crate::shared::durable::ClientSseStream,
    deadline: std::time::Instant,
) -> Option<Result<crate::shared::durable::SseEvent, TransportError>> {
    futures_lite::future::poll_fn(|cx| match stream.as_mut().poll_next(cx) {
        std::task::Poll::Pending => {
            if std::time::Instant::now() >= deadline {
                std::task::Poll::Ready(None)
            } else {
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
        }
        std::task::Poll::Ready(item) => std::task::Poll::Ready(item),
    })
    .await
}

fn outcome(path: &str, response: ClientResponse) -> Result<AppendResult, ClientError> {
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
        duplicate: response.status == 204,
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
                Err(ClientError::Conflict(bounded_body(&response)))
            }
        }
        status => Err(unexpected(status, &response)),
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
