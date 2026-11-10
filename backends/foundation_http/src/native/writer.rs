//! Response writer — the one way a handler writes bytes to a non-blocking
//! connection.
//!
//! WHY: The connection socket is non-blocking. A write can take only part of a
//! buffer and then fail with `WouldBlock` when the send buffer is full. Code
//! that retries `write_all(&whole_buffer)` restarts at byte 0 and sends
//! duplicate bytes, or gives up and truncates the response. Code that sleeps
//! to retry holds a worker thread while a slow client drains the socket.
//!
//! WHAT: [`ResponseWriter`] writes a byte slice to the connection, tracks the
//! offset the socket took, and on `WouldBlock` parks the task on write
//! readiness. [`WriterConfig`] holds the limits. The `send_*` helpers write a
//! complete in-memory response and return a [`ConnectionResult::Async`].
//!
//! HOW: A kernel-fd transport parks on the shared reactor through
//! [`FdReadiness`]; the overlay parks on a one-shot wake callback its driver
//! fires; a transport that cannot report readiness parks on the valtron timer
//! for one `poll_fallback_interval` and the caller retries the write. The
//! reactor watch is registered on the first `WouldBlock` only (most small
//! responses never block) and is removed when the writer drops
//! (`FdReadiness` deregisters in its `Drop`). `send_timeout` has nginx
//! `send_timeout` meaning: the time between two successive writes. The timer
//! restarts after every write that takes at least one byte, so a long download
//! to a slow but live client is never cut, and a client that stops reading is
//! closed after `send_timeout`.

use std::future::{poll_fn, Future};
use std::io::{self, Write};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::timer::{self, Sleep};
use foundation_iogate::FdReadiness;
use foundation_netio::native::connection::{ConnReadiness, ConnWaker};
use foundation_netio::native::readiness::IoInterest;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{
    Http11, RenderHttp, SendSafeBody, SimpleHeader, SimpleOutgoingResponse,
};
use serde::Serialize;

use crate::shared::serve::{status_from_code, ConnectionResult, ServeOutcome};

/// Limits for [`ResponseWriter`].
///
/// Each limit has a builder method and a documented default. Handlers that own
/// a config ([`StaticFileConfig`](crate::native::handlers::static_file::StaticFileConfig),
/// [`StaticAssetHandler`](crate::native::handlers::static_asset::StaticAssetHandler))
/// embed one; the `send_*` helpers use the default.
#[derive(Clone, Copy, Debug)]
pub struct WriterConfig {
    /// The longest the socket may take no byte of a pending write before the
    /// connection is closed. The timer restarts after every write that takes at
    /// least one byte. Default 60 s, nginx's `send_timeout`.
    pub send_timeout: Duration,
    /// How often the writer retries on a transport that cannot report write
    /// readiness (a completion socket, the openssl or native-tls backends). The
    /// wait is a timer wake, never a thread sleep. Default 10 ms, the same as
    /// the `ConnectRPC` server's fallback.
    pub poll_fallback_interval: Duration,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            send_timeout: Duration::from_secs(60),
            poll_fallback_interval: Duration::from_millis(10),
        }
    }
}

impl WriterConfig {
    /// The default limits. See the field docs for each default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set [`Self::send_timeout`].
    #[must_use]
    pub fn with_send_timeout(mut self, timeout: Duration) -> Self {
        self.send_timeout = timeout;
        self
    }

    /// Set [`Self::poll_fallback_interval`].
    #[must_use]
    pub fn with_poll_fallback_interval(mut self, interval: Duration) -> Self {
        self.poll_fallback_interval = interval;
        self
    }
}

/// Writes to one connection without truncating, repeating or sleeping. See the
/// module docs.
pub struct ResponseWriter {
    conn: SharedByteBufferStream<RawStream>,
    config: WriterConfig,
    /// The context whose clock measures the send timeout and the retry interval.
    ctx: Ctx,
    /// The timer for `send_timeout` from the last progress; cleared on every
    /// progress, so an idle writer is not timed out.
    idle_timer: Option<Sleep>,
    /// Built on the first `WouldBlock`.
    wait: Option<WaitSource>,
}

/// How to wait for write readiness on this connection.
struct WaitSource {
    readiness: Option<ConnReadiness>,
    fd_ready: Option<FdReadiness>,
    fallback: Option<Sleep>,
}

impl ResponseWriter {
    /// A writer for `conn` with the given limits. Its timers read the clock of `ctx`.
    #[must_use]
    pub fn new(conn: SharedByteBufferStream<RawStream>, config: WriterConfig, ctx: Ctx) -> Self {
        Self {
            conn,
            config,
            ctx,
            idle_timer: None,
            wait: None,
        }
    }

    /// Write all of `buf`. The offset advances by what the socket took, so a
    /// partial write never repeats bytes. `WouldBlock` parks on write readiness.
    ///
    /// Each call starts the idle timer again. A writer that stayed idle between
    /// writes (a long-lived gRPC stream, a keep-alive response) must not time
    /// out on the first blocked write of a new response: the timer measures a
    /// stall inside an operation, not the writer's age.
    ///
    /// # Errors
    /// The connection failed (`ConnectionReset`, `BrokenPipe`, ...), or the
    /// socket took no byte for `send_timeout` (`TimedOut`).
    pub async fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.progress();
        let mut offset = 0;
        while offset < buf.len() {
            match self.conn.write(&buf[offset..]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => {
                    offset += n;
                    self.progress();
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => self.writable().await?,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Flush the stream's own buffer. `WouldBlock` parks on write readiness; the
    /// buffer keeps what is unwritten, so a retry resumes where it stopped.
    ///
    /// A non-blocking `flush` drains part of that buffer and then returns
    /// `WouldBlock`. That is progress: the idle timer restarts whenever the
    /// buffer shrinks, so a large flush to a slow but live client is not cut
    /// off while it is moving bytes.
    ///
    /// # Errors
    /// As [`Self::write_all`].
    pub async fn flush(&mut self) -> io::Result<()> {
        // A flush is an operation of its own; see `write_all`.
        self.progress();
        loop {
            let before = self.write_buffered_len();
            match self.conn.flush() {
                Ok(()) => return Ok(()),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if self.write_buffered_len() < before {
                        self.progress();
                    }
                    self.writable().await?;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }

    /// How many bytes the stream still buffers for writing.
    fn write_buffered_len(&self) -> usize {
        self.conn.with_inner_ref(RawStream::write_buffer_len)
    }

    /// The socket took at least one byte: restart the idle timer.
    fn progress(&mut self) {
        // Re-arming from "now" on the next `WouldBlock` restarts the deadline;
        // the timer itself runs on the valtron timer thread (GitHub issue #102).
        self.idle_timer = None;
    }

    async fn writable(&mut self) -> io::Result<()> {
        poll_fn(|cx| self.poll_writable(cx)).await
    }

    fn poll_writable(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let send_timeout = self.config.send_timeout;
        let timer = self
            .idle_timer
            .get_or_insert_with(|| timer::sleep_for(send_timeout, &self.ctx));
        // The timer fires at the deadline; no separate clock read is needed.
        if Pin::new(timer).poll(cx).is_ready() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the client took no byte of the response within the send timeout",
            )));
        }
        let conn = &self.conn;
        let wait = self.wait.get_or_insert_with(|| WaitSource::new(conn));
        wait.poll(cx, self.config.poll_fallback_interval, &self.ctx)
    }
}

impl WaitSource {
    fn new(conn: &SharedByteBufferStream<RawStream>) -> Self {
        let readiness = conn.with_inner_ref(RawStream::conn_readiness);
        let fd_ready = match &readiness {
            Some(ConnReadiness::Fd(fd, _)) => match FdReadiness::register(*fd) {
                Ok(r) => Some(r),
                Err(e) => {
                    tracing::debug!("response writer: cannot watch the socket on the reactor: {e}");
                    None
                }
            },
            _ => None,
        };
        // Without a reactor registration a kernel-fd transport falls back to
        // timer-paced retries, like a transport that reports no readiness.
        let readiness = match (&readiness, &fd_ready) {
            (Some(ConnReadiness::Fd(..)), None) => None,
            _ => readiness,
        };
        Self {
            readiness,
            fd_ready,
            fallback: None,
        }
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
        fallback_interval: Duration,
        ctx: &Ctx,
    ) -> Poll<io::Result<()>> {
        if let Some(readiness) = &self.readiness {
            // Register before the readiness probe, so an edge between the two
            // still wakes this task.
            match readiness {
                ConnReadiness::Fd(..) => {
                    let (Some(fd_ready), Some((_, corrected))) =
                        (&self.fd_ready, readiness.poll_fd(IoInterest::Writable))
                    else {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::Unsupported,
                            "this transport cannot report write readiness",
                        )));
                    };
                    fd_ready.clear();
                    if let Err(e) = fd_ready.set_writable(corrected == IoInterest::Writable) {
                        return Poll::Ready(Err(e));
                    }
                    if !fd_ready.register_waker(cx.waker()) {
                        // The reactor no longer holds this registration. Parking
                        // would then never wake, so report it instead of hanging.
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::NotConnected,
                            "the reactor no longer watches this connection",
                        )));
                    }
                }
                ConnReadiness::Overlay(_) => {
                    let waker = cx.waker().clone();
                    let wake: ConnWaker = Arc::new(move || waker.wake_by_ref());
                    readiness.register_waker(IoInterest::Writable, wake);
                }
            }
            match readiness.ready_now(IoInterest::Writable) {
                Ok(true) => Poll::Ready(Ok(())),
                Ok(false) => Poll::Pending,
                Err(e) => Poll::Ready(Err(e)),
            }
        } else {
            // No readiness source: park on the timer for one interval, then
            // let the caller retry its write.
            let fallback = self
                .fallback
                .get_or_insert_with(|| timer::sleep_for(fallback_interval, ctx));
            if Pin::new(fallback).poll(cx).is_ready() {
                self.fallback = None;
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Whole in-memory responses

/// Render the head of a response with `Content-Length: len` and no body.
///
/// # Errors
/// The response could not be built or rendered.
pub fn render_head(
    status: u16,
    headers: &[(SimpleHeader, String)],
    len: usize,
) -> Result<Vec<u8>, String> {
    let mut builder = SimpleOutgoingResponse::builder().with_status(status_from_code(status));
    for (name, value) in headers {
        builder = builder.add_header(name.clone(), value.clone());
    }
    let response = builder
        .add_header(SimpleHeader::CONTENT_LENGTH, len.to_string())
        .with_body(SendSafeBody::None)
        .build()
        .map_err(|e| e.to_string())?;
    let mut head = Vec::new();
    for chunk in Http11::response(response)
        .http_render()
        .map_err(|e| e.to_string())?
    {
        head.extend_from_slice(&chunk.map_err(|e| e.to_string())?);
    }
    Ok(head)
}

/// Render a complete response (head and any in-memory body) to wire bytes.
///
/// WHY: callers that build a response with custom headers — an upgrade head, an
/// interim `100 Continue`, an ACME answer — must still write it through
/// [`ResponseWriter`]. This turns the built response into the bytes that writer
/// takes, without adding or removing a header.
///
/// # Errors
/// The response could not be rendered.
pub fn render_message(response: SimpleOutgoingResponse) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for chunk in Http11::response(response)
        .http_render()
        .map_err(|e| e.to_string())?
    {
        out.extend_from_slice(&chunk.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// Write `head` then `body` (already in memory, not copied) and report what the
/// connection does next: `Keep` on success, `Close` on any failure.
pub async fn write_response<B: AsRef<[u8]>>(
    mut writer: ResponseWriter,
    head: Vec<u8>,
    body: B,
) -> ServeOutcome {
    let result = async {
        writer.write_all(&head).await?;
        writer.write_all(body.as_ref()).await?;
        writer.flush().await
    }
    .await;
    match result {
        Ok(()) => ServeOutcome::Keep,
        Err(e) => {
            log_write_failure(&e);
            ServeOutcome::Close(None)
        }
    }
}

/// Log why a response write ended, at the level that fits the cause.
pub(crate) fn log_write_failure(e: &io::Error) {
    match e.kind() {
        io::ErrorKind::ConnectionReset
        | io::ErrorKind::BrokenPipe
        | io::ErrorKind::ConnectionAborted => {
            tracing::debug!("response write: peer closed: {e}");
        }
        io::ErrorKind::TimedOut => tracing::debug!("response write: send timeout: {e}"),
        _ => tracing::error!("response write failed: {e}"),
    }
}

/// Send a complete in-memory response. The returned result is
/// `ConnectionResult::Async`; the connection keeps going when the write ends.
///
/// The `Connection: close` header matches [`respond`](crate::shared::serve::respond).
#[must_use]
pub fn send_bytes<B: AsRef<[u8]> + Send + Sync + 'static>(
    conn: SharedByteBufferStream<RawStream>,
    status: u16,
    content_type: &str,
    body: B,
    ctx: &Ctx,
) -> ConnectionResult {
    let headers = [
        (SimpleHeader::CONTENT_TYPE, content_type.to_string()),
        (SimpleHeader::CONNECTION, "close".to_string()),
    ];
    match render_head(status, &headers, body.as_ref().len()) {
        Ok(head) => {
            let writer = ResponseWriter::new(conn, WriterConfig::default(), ctx.clone());
            ConnectionResult::Async(Box::pin(write_response(writer, head, body)))
        }
        Err(e) => {
            tracing::error!("response build failed: {e}");
            ConnectionResult::Close(None)
        }
    }
}

/// Send a plain text response. See [`send_bytes`].
#[must_use]
pub fn send_text(
    conn: SharedByteBufferStream<RawStream>,
    status: u16,
    body: &str,
    ctx: &Ctx,
) -> ConnectionResult {
    send_bytes(conn, status, "text/plain", body.as_bytes().to_vec(), ctx)
}

/// Send an HTML response. See [`send_bytes`].
#[must_use]
pub fn send_html(
    conn: SharedByteBufferStream<RawStream>,
    status: u16,
    body: &str,
    ctx: &Ctx,
) -> ConnectionResult {
    send_bytes(conn, status, "text/html", body.as_bytes().to_vec(), ctx)
}

/// Send a JSON response. See [`send_bytes`].
#[must_use]
pub fn send_json<T: Serialize>(
    conn: SharedByteBufferStream<RawStream>,
    status: u16,
    body: &T,
    ctx: &Ctx,
) -> ConnectionResult {
    match serde_json::to_vec(body) {
        Ok(bytes) => send_bytes(conn, status, "application/json", bytes, ctx),
        Err(e) => {
            tracing::error!("response JSON encode failed: {e}");
            ConnectionResult::Close(None)
        }
    }
}

/// Send a 404 Not Found response. See [`send_bytes`].
#[must_use]
pub fn send_not_found(conn: SharedByteBufferStream<RawStream>, ctx: &Ctx) -> ConnectionResult {
    send_text(conn, 404, "Not Found", ctx)
}

/// Send a 500 Internal Server Error response. See [`send_bytes`].
#[must_use]
pub fn send_server_error(
    conn: SharedByteBufferStream<RawStream>,
    reason: Option<&str>,
    ctx: &Ctx,
) -> ConnectionResult {
    send_text(conn, 500, reason.unwrap_or("Internal Server Error"), ctx)
}

/// Send a `100 Continue` interim response. See [`send_bytes`].
///
/// A `100 Continue` is a header-less interim response: the `Http11` renderer
/// allows an informational (1xx) status without headers. The response is
/// flushed so the client can send its body at once.
#[must_use]
pub fn send_continue_100(conn: SharedByteBufferStream<RawStream>, ctx: &Ctx) -> ConnectionResult {
    let response = match SimpleOutgoingResponse::builder()
        .with_status(status_from_code(100))
        .with_body(SendSafeBody::None)
        .build()
    {
        Ok(response) => response,
        Err(e) => {
            tracing::error!("100 Continue build failed: {e}");
            return ConnectionResult::Close(None);
        }
    };
    match render_message(response) {
        Ok(head) => {
            let writer = ResponseWriter::new(conn, WriterConfig::default(), ctx.clone());
            ConnectionResult::Async(Box::pin(write_response(writer, head, [])))
        }
        Err(e) => {
            tracing::error!("100 Continue render failed: {e}");
            ConnectionResult::Close(None)
        }
    }
}
