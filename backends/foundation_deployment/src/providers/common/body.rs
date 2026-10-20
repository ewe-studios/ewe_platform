//! Streaming response bodies for the generated API clients.
//!
//! WHY: Some endpoints answer with a body of unbounded size — a tar archive, a
//! followed log, an event feed, a JSON-lines progress stream that the server
//! writes *while* it does the work. Reading such a body whole into memory risks
//! running out of it, and never returns at all for a stream that does not end.
//! Error bodies are read for their message, but only so much of them.
//!
//! WHAT: Native async streams over a response body, for the generated
//! `*_request` functions to hand back unread:
//!
//! - [`ByteStream`] — the body as raw chunks (`application/x-tar`,
//!   `application/octet-stream`, multiplexed log streams, ...).
//! - [`JsonLinesStream`] — one typed item per line (`application/jsonl` and
//!   kin), parsed as the lines arrive.
//!
//! plus [`read_error_body`], which reads at most a given number of bytes of a
//! non-2xx body, [`with_raw_body`], which attaches a non-JSON request body,
//! and [`BodyLimits`], the configurable caps on both.
//!
//! HOW: Both streams sit on netio's [`AsyncSendSafeBody`] and implement
//! [`futures_core::Stream`]; each also has an inherent `next().await`, so a
//! caller needs no extension trait. Each holds only what it has not handed out
//! yet: a [`JsonLinesStream`] keeps the bytes of the line it is assembling,
//! bounded by its configured line limit ([`BodyLimits::max_json_line`]).
//! Dropping a stream drops the body, which closes the connection.

use core::future::poll_fn;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{Context, Poll};

use bytes::Bytes;
use foundation_netio::shared::client::body_reader::AsyncSendSafeBody;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader};
use foundation_netio::PreparedRequestBuilder;
use futures_core::Stream as FuturesStream;
use serde::de::DeserializeOwned;

use super::api_types::ApiError;

/// Caps on how much of a response body is held in memory at once.
///
/// None of these limits how long a streamed body may be: a [`ByteStream`] or
/// a [`JsonLinesStream`] reads as far as it is asked to. They cap only what
/// has to be held whole — an error body read for its message, one line of a
/// JSON-lines body.
///
/// The generated API clients read their responses under the limits of the
/// [`ApiClient`](super::ApiClient) they are given; a bare `DynNetClient` gets
/// [`BodyLimits::default`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyLimits {
    /// The most of a non-2xx response's body read for its error message;
    /// the rest is never read.
    ///
    /// Default 64 KiB. API error bodies are a short JSON object
    /// (`{"message": …}`), so this keeps every real message whole while
    /// bounding a broken or hostile one. It is the order of what nginx holds
    /// of a proxied response in memory by default (`proxy_buffers 8 4k|8k`,
    /// 32–64 KiB) before spilling to disk; Docker's own Go client allows
    /// 1 MiB.
    pub max_error_body: usize,
    /// The longest line a [`JsonLinesStream`] buffers while it waits for the
    /// newline. A line still growing past this is reported as an error
    /// rather than buffered without bound.
    ///
    /// Default 1 MiB: Go `net/http`'s `DefaultMaxHeaderBytes`, and the
    /// `foundation_netio` client's own cap on one line of a line-delimited
    /// body (`FrameLimits::max_line`). A progress or event line is a few
    /// hundred bytes and a stats sample a few kilobytes.
    pub max_json_line: usize,
}

impl Default for BodyLimits {
    fn default() -> Self {
        Self {
            max_error_body: 64 * 1024,
            max_json_line: 1024 * 1024,
        }
    }
}

impl BodyLimits {
    /// Read at most `bytes` of an error body.
    #[must_use]
    pub fn with_max_error_body(mut self, bytes: usize) -> Self {
        self.max_error_body = bytes;
        self
    }

    /// Buffer at most `bytes` of one JSON-lines line.
    #[must_use]
    pub fn with_max_json_line(mut self, bytes: usize) -> Self {
        self.max_json_line = bytes;
        self
    }
}

/// Read at most `limit` bytes of `body` as (lossy) UTF-8; the rest is never
/// read.
///
/// # Panics
///
/// Never panics.
pub async fn read_bounded(body: SendSafeBody, limit: usize) -> String {
    let mut body = AsyncSendSafeBody::from(body);
    let mut buf = Vec::new();
    while buf.len() < limit {
        match poll_fn(|cx| Pin::new(&mut body).poll_next(cx)).await {
            Some(Ok(chunk)) => {
                let room = limit - buf.len();
                buf.extend_from_slice(&chunk[..chunk.len().min(room)]);
            }
            // A body that fails part-way still says what arrived before.
            Some(Err(e)) => {
                tracing::debug!("error response body ended early: {e}");
                break;
            }
            None => break,
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

/// The message of a non-2xx response: at most `limit` bytes of its body
/// (see [`BodyLimits::max_error_body`]), or `None` when the body is empty.
///
/// # Panics
///
/// Never panics.
pub async fn read_error_body(body: SendSafeBody, limit: usize) -> Option<String> {
    let text = read_bounded(body, limit).await;
    (!text.is_empty()).then_some(text)
}

/// Attach a non-JSON request body (a tar archive, raw bytes, plain text) sent
/// as `media_type`.
///
/// `Content-Length` is set for an in-memory body; a streamed one is sent as
/// the transport frames it. The generated functions apply their `builder_mod`
/// after this, so a caller can still replace the `Content-Type` with a more
/// specific one (`application/x-tar` for a body declared as
/// `application/octet-stream`).
#[must_use]
pub fn with_raw_body(
    mut builder: PreparedRequestBuilder,
    media_type: &str,
    body: SendSafeBody,
) -> PreparedRequestBuilder {
    let length = match &body {
        SendSafeBody::None => Some(0),
        SendSafeBody::Text(text) => Some(text.len()),
        SendSafeBody::Bytes(bytes) => Some(bytes.len()),
        SendSafeBody::Stream(_)
        | SendSafeBody::ChunkedStream(_)
        | SendSafeBody::LineFeedStream(_)
        | SendSafeBody::SseStream(_) => None,
    };
    builder = builder.header(SimpleHeader::CONTENT_TYPE, media_type);
    if let Some(length) = length {
        builder = builder.header(SimpleHeader::CONTENT_LENGTH, length.to_string());
    }
    builder.body(body)
}

/// The error for a body that failed part-way through.
fn body_read_failed(e: impl core::fmt::Display) -> ApiError {
    ApiError::RequestSendFailed(format!("reading the response body: {e}"))
}

// ── ByteStream ──────────────────────────────────────────────────────────

/// A response body as raw chunks, read as they are asked for.
///
/// Ends after the body ends or after the first error.
pub struct ByteStream {
    body: Option<AsyncSendSafeBody>,
}

impl ByteStream {
    /// Wrap a body that has not been read yet.
    #[must_use]
    pub fn new(body: SendSafeBody) -> Self {
        Self {
            body: Some(AsyncSendSafeBody::from(body)),
        }
    }

    /// The next chunk, `None` once the body has ended.
    ///
    /// # Errors
    ///
    /// [`ApiError::RequestSendFailed`] if reading the body fails; the stream
    /// ends after it.
    pub async fn next(&mut self) -> Option<Result<Bytes, ApiError>> {
        poll_fn(|cx| Pin::new(&mut *self).poll_next(cx)).await
    }

    /// Read the rest of the body, discarding it, and return how many bytes
    /// there were.
    ///
    /// # Errors
    ///
    /// [`ApiError::RequestSendFailed`] if reading the body fails.
    pub async fn drain(mut self) -> Result<u64, ApiError> {
        let mut total = 0u64;
        while let Some(chunk) = self.next().await {
            total += chunk?.len() as u64;
        }
        Ok(total)
    }
}

impl core::fmt::Debug for ByteStream {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ByteStream")
            .field("ended", &self.body.is_none())
            .finish_non_exhaustive()
    }
}

impl FuturesStream for ByteStream {
    type Item = Result<Bytes, ApiError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let Some(body) = this.body.as_mut() else {
            return Poll::Ready(None);
        };
        match Pin::new(body).poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok(chunk))) => Poll::Ready(Some(Ok(Bytes::from(chunk)))),
            Poll::Ready(Some(Err(e))) => {
                this.body = None;
                Poll::Ready(Some(Err(body_read_failed(e))))
            }
            Poll::Ready(None) => {
                this.body = None;
                Poll::Ready(None)
            }
        }
    }
}

// ── JsonLinesStream ─────────────────────────────────────────────────────

/// A JSON-lines response body, one `T` per line, parsed as the lines arrive.
///
/// Blank lines are skipped; a final line without a trailing newline is still
/// a line. Ends after the body ends or after the first error: a line that is
/// not a valid `T` ([`ApiError::ParseFailed`]), a line longer than its
/// `max_line` (also `ParseFailed`), or a failed read
/// ([`ApiError::RequestSendFailed`]).
pub struct JsonLinesStream<T> {
    bytes: ByteStream,
    /// Bytes of lines not yet handed out: `buf[start..]`.
    buf: Vec<u8>,
    start: usize,
    /// How far into `buf` has been searched for a newline.
    scanned: usize,
    /// The longest line buffered before it is an error.
    max_line: usize,
    done: bool,
    _item: PhantomData<fn() -> T>,
}

impl<T: DeserializeOwned> JsonLinesStream<T> {
    /// Wrap a body that has not been read yet, buffering at most `max_line`
    /// bytes of one line (see [`BodyLimits::max_json_line`]).
    #[must_use]
    pub fn new(body: SendSafeBody, max_line: usize) -> Self {
        Self {
            bytes: ByteStream::new(body),
            buf: Vec::new(),
            start: 0,
            scanned: 0,
            max_line,
            done: false,
            _item: PhantomData,
        }
    }

    /// The next item, `None` once the body has ended.
    ///
    /// # Errors
    ///
    /// See [`JsonLinesStream`]; the stream ends after an error.
    pub async fn next(&mut self) -> Option<Result<T, ApiError>> {
        poll_fn(|cx| Pin::new(&mut *self).poll_next(cx)).await
    }

    /// Take the next complete line out of the buffer, if one has arrived.
    fn take_line(&mut self) -> Option<Vec<u8>> {
        let newline = self.buf[self.scanned..].iter().position(|b| *b == b'\n')?;
        let end = self.scanned + newline;
        let line = self.buf[self.start..end].to_vec();
        self.start = end + 1;
        self.scanned = self.start;
        // Drop consumed bytes once they are the larger part of the buffer.
        if self.start > self.buf.len() / 2 {
            self.buf.drain(..self.start);
            self.start = 0;
            self.scanned = 0;
        }
        Some(line)
    }

    /// The error for a line longer than `max_line`; it ends the stream.
    fn line_too_long(&mut self) -> ApiError {
        self.done = true;
        ApiError::ParseFailed(format!(
            "response line longer than {} bytes",
            self.max_line
        ))
    }

    /// Parse one line into the next item; a failure ends the stream.
    ///
    /// A line over `max_line` is an error however it arrived: one that came
    /// whole in a single chunk is refused as one still growing would be, so
    /// the limit does not depend on how the body happened to be chunked.
    fn parse(&mut self, line: &[u8]) -> Result<T, ApiError> {
        if line.len() > self.max_line {
            return Err(self.line_too_long());
        }
        serde_json::from_slice(line).map_err(|e| {
            self.done = true;
            ApiError::ParseFailed(format!(
                "response line {:?}: {e}",
                String::from_utf8_lossy(&line[..line.len().min(256)])
            ))
        })
    }
}

impl<T> core::fmt::Debug for JsonLinesStream<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("JsonLinesStream")
            .field("item", &core::any::type_name::<T>())
            .field("buffered", &(self.buf.len() - self.start))
            .field("max_line", &self.max_line)
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}

impl<T: DeserializeOwned> FuturesStream for JsonLinesStream<T> {
    type Item = Result<T, ApiError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if this.done {
                return Poll::Ready(None);
            }
            if let Some(line) = this.take_line() {
                if line.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                return Poll::Ready(Some(this.parse(&line)));
            }
            this.scanned = this.buf.len();
            if this.buf.len() - this.start > this.max_line {
                return Poll::Ready(Some(Err(this.line_too_long())));
            }
            match Pin::new(&mut this.bytes).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(chunk))) => this.buf.extend_from_slice(&chunk),
                Poll::Ready(Some(Err(e))) => {
                    this.done = true;
                    return Poll::Ready(Some(Err(e)));
                }
                // End of body: what is left is a last line without its newline.
                Poll::Ready(None) => {
                    this.done = true;
                    let rest = this.buf.split_off(this.start);
                    this.buf.clear();
                    this.start = 0;
                    this.scanned = 0;
                    if rest.iter().all(u8::is_ascii_whitespace) {
                        return Poll::Ready(None);
                    }
                    return Poll::Ready(Some(this.parse(&rest)));
                }
            }
        }
    }
}
