//! Static file handler — serves files from a directory with path traversal prevention.
//!
//! WHY: A file can be far larger than the socket send buffer. The handler must
//! stream it from disk, write every byte exactly once and in order, and never
//! hold a worker thread while a slow client drains the socket.
//!
//! WHAT: [`StaticFileHandler`] answers `GET`/`HEAD` for files under a root
//! directory. [`StaticFileConfig`] holds its limits (chunk size, write
//! deadline).
//!
//! HOW: `serve` checks the path, opens the file and reads its length, then
//! returns [`ConnectionResult::Async`]. The future writes the head and then the
//! body one chunk at a time, tracking how much of each buffer the socket has
//! taken. On `WouldBlock` it parks on socket write readiness (the shared
//! reactor, or the overlay's wake callback) and never sleeps a thread. A reset,
//! aborted or broken connection ends the future at once.

use std::fs::File;
use std::future::{poll_fn, Future};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::timer::{self, Sleep};
use foundation_iogate::FdReadiness;
use foundation_netio::native::connection::{ConnReadiness, ConnWaker};
use foundation_netio::native::readiness::IoInterest;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{
    Http11, RenderHttp, SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};

use crate::shared::context::ContextBag;
use crate::shared::serve::{ConnectionResult, Serve, ServeFactory};

/// MIME type detection based on file extension.
fn mime_type(path: &Path) -> &str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html" | "htm") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("txt") => "text/plain",
        Some("xml") => "application/xml",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// Limits for [`StaticFileHandler`].
///
/// Each limit has a builder method and a documented default.
#[derive(Clone, Debug)]
pub struct StaticFileConfig {
    /// Bytes read from the file and written to the socket per step. Default
    /// 64 KiB, the order of nginx's `output_buffers` (2 x 32 KiB) and of Go's
    /// `io.Copy` buffer. A value of 0 is raised to 1.
    pub chunk_size: usize,
    /// The longest a whole response may take to write, head and body. A client
    /// that reads slower than this is closed. Default 60 s, nginx's
    /// `send_timeout`.
    pub write_timeout: Duration,
    /// How often the handler retries a write on a transport that cannot report
    /// write readiness (a completion socket, the openssl or native-tls
    /// backends). The wait is a timer wake, never a thread sleep. Default
    /// 10 ms, the same as the `ConnectRPC` server's fallback.
    pub poll_fallback_interval: Duration,
}

impl Default for StaticFileConfig {
    fn default() -> Self {
        Self {
            chunk_size: 64 * 1024,
            write_timeout: Duration::from_secs(60),
            poll_fallback_interval: Duration::from_millis(10),
        }
    }
}

impl StaticFileConfig {
    /// The default limits. See the field docs for each default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set [`Self::chunk_size`]. 0 is raised to 1.
    #[must_use]
    pub fn with_chunk_size(mut self, bytes: usize) -> Self {
        self.chunk_size = bytes.max(1);
        self
    }

    /// Set [`Self::write_timeout`].
    #[must_use]
    pub fn with_write_timeout(mut self, timeout: Duration) -> Self {
        self.write_timeout = timeout;
        self
    }

    /// Set [`Self::poll_fallback_interval`].
    #[must_use]
    pub fn with_poll_fallback_interval(mut self, interval: Duration) -> Self {
        self.poll_fallback_interval = interval;
        self
    }
}

/// Handler that serves static files from a root directory.
pub struct StaticFileHandler {
    root: PathBuf,
    config: StaticFileConfig,
}

impl StaticFileHandler {
    /// Create a new `StaticFileHandler` serving from the given root directory,
    /// with the default [`StaticFileConfig`].
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_config(root, StaticFileConfig::default())
    }

    /// Create a handler serving `root` with the given limits.
    #[must_use]
    pub fn with_config(root: impl Into<PathBuf>, config: StaticFileConfig) -> Self {
        Self {
            root: root.into(),
            config,
        }
    }

    /// The limits this handler uses.
    #[must_use]
    pub fn config(&self) -> &StaticFileConfig {
        &self.config
    }

    fn serve_file(
        &self,
        path: &str,
        head_only: bool,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let mut conn = conn;
        // Prevent path traversal
        let safe_path = path.trim_start_matches('/');
        if safe_path.contains("..") {
            return crate::shared::serve::respond::text(&mut conn, 403, "Forbidden")
                .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep);
        }

        let file_path = self.root.join(safe_path);

        // Verify the resolved path is still under root
        if !file_path.starts_with(&self.root) {
            return crate::shared::serve::respond::text(&mut conn, 403, "Forbidden")
                .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep);
        }

        // Check if it's a directory — try index.html
        if file_path.is_dir() {
            let index_path = file_path.join("index.html");
            return self.serve_file_path(&index_path, head_only, conn);
        }

        self.serve_file_path(&file_path, head_only, conn)
    }

    fn serve_file_path(
        &self,
        path: &Path,
        head_only: bool,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let opened = File::open(path).and_then(|file| {
            let meta = file.metadata()?;
            Ok((file, meta))
        });
        let (file, meta) = match opened {
            Ok((file, meta)) if meta.is_file() => (file, meta),
            Ok(_) => {
                return crate::shared::serve::respond::not_found(&mut conn)
                    .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep);
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return crate::shared::serve::respond::not_found(&mut conn)
                    .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep);
            }
            Err(e) => {
                tracing::error!("Failed to open static file {path:?}: {e}");
                return crate::shared::serve::respond::server_error(&mut conn, None)
                    .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep);
            }
        };
        let len = meta.len();

        // The head is small; render it now so a render failure closes the
        // connection before any byte is written.
        let response = SimpleOutgoingResponse::builder()
            .with_status(Status::OK)
            .add_header(SimpleHeader::CONTENT_TYPE, mime_type(path).to_string())
            .add_header(SimpleHeader::CONTENT_LENGTH, len.to_string())
            .with_body(SendSafeBody::None)
            .build();
        let response = match response {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("Failed to build static file response: {e}");
                return ConnectionResult::Close(None);
            }
        };
        let mut head = Vec::new();
        match Http11::response(response).http_render() {
            Ok(chunks) => {
                for chunk in chunks {
                    match chunk {
                        Ok(bytes) => head.extend_from_slice(&bytes),
                        Err(e) => {
                            tracing::error!("Failed to render static file head: {e}");
                            return ConnectionResult::Close(None);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to render static file response: {e}");
                return ConnectionResult::Close(None);
            }
        }

        let waiter = WriteWaiter::new(
            &conn,
            Instant::now() + self.config.write_timeout,
            self.config.poll_fallback_interval,
        );
        let job = SendFile {
            conn,
            waiter,
            head,
            file,
            len: if head_only { 0 } else { len },
            chunk_size: self.config.chunk_size,
            path: path.to_path_buf(),
        };
        ConnectionResult::Async(Box::pin(job.run()))
    }
}

impl ServeFactory for StaticFileHandler {
    fn create(_bag: &ContextBag) -> Self {
        panic!("StaticFileHandler requires a root directory — use StaticFileHandler::new() and register manually");
    }
}

impl Serve for StaticFileHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let full_url = &req.request_url.url;
        let path = full_url.split('?').next().unwrap_or(full_url);
        let head_only = matches!(req.method, SimpleMethod::HEAD);
        self.serve_file(path, head_only, conn)
    }
}

// ---------------------------------------------------------------------------
// The response write

/// One response in flight: the head, then `len` bytes of `file`.
struct SendFile {
    conn: SharedByteBufferStream<RawStream>,
    waiter: WriteWaiter,
    head: Vec<u8>,
    file: File,
    /// Body bytes to send (0 for a `HEAD` request).
    len: u64,
    chunk_size: usize,
    path: PathBuf,
}

impl SendFile {
    async fn run(mut self) -> ConnectionResult {
        match self.send().await {
            Ok(()) => ConnectionResult::Keep,
            Err(e) => {
                match e.kind() {
                    io::ErrorKind::ConnectionReset
                    | io::ErrorKind::BrokenPipe
                    | io::ErrorKind::ConnectionAborted => {
                        tracing::debug!(path = ?self.path, "static file: peer closed: {e}");
                    }
                    io::ErrorKind::TimedOut => {
                        tracing::debug!(path = ?self.path, "static file: write deadline passed: {e}");
                    }
                    _ => tracing::error!(path = ?self.path, "static file: write failed: {e}"),
                }
                ConnectionResult::Close(None)
            }
        }
    }

    async fn send(&mut self) -> io::Result<()> {
        let head = std::mem::take(&mut self.head);
        self.write_all(&head).await?;

        // The file is read inline, one chunk at a time. A chunk read hits the
        // page cache and returns in microseconds; the crate has no file I/O
        // offload pool, and a new pool is not worth one chunk read.
        let mut buf = vec![0u8; self.chunk_size];
        let mut remaining = self.len;
        while remaining > 0 {
            let want = usize::try_from(remaining).map_or(buf.len(), |r| r.min(buf.len()));
            let n = match self.file.read(&mut buf[..want]) {
                Ok(0) => {
                    // The file shrank after its length was read: the promised
                    // Content-Length cannot be met, so the connection must close.
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "the file ended before its length",
                    ));
                }
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            self.write_all(&buf[..n]).await?;
            remaining -= n as u64;
        }
        self.flush().await
    }

    /// Write all of `buf`. The offset advances by what the socket took, so a
    /// partial write never repeats bytes. `WouldBlock` parks on write readiness.
    async fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let mut offset = 0;
        while offset < buf.len() {
            match self.conn.write(&buf[offset..]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => offset += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.waiter.writable().await?;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Flush the stream's own buffer. `WouldBlock` parks on write readiness; the
    /// buffer keeps what is unwritten, so a retry resumes where it stopped.
    async fn flush(&mut self) -> io::Result<()> {
        loop {
            match self.conn.flush() {
                Ok(()) => return Ok(()),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.waiter.writable().await?;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }
}

/// Waits for a connection to become writable, bounded by one deadline for the
/// whole response.
///
/// A kernel-fd transport parks on the shared reactor through [`FdReadiness`];
/// the overlay parks on a one-shot wake callback its driver fires; a transport
/// that cannot report readiness parks on the valtron timer for one
/// `poll_fallback_interval` and lets the caller retry the write.
struct WriteWaiter {
    readiness: Option<ConnReadiness>,
    fd_ready: Option<FdReadiness>,
    deadline: Sleep,
    deadline_at: Instant,
    poll_fallback_interval: Duration,
    fallback: Option<Sleep>,
}

impl WriteWaiter {
    fn new(
        conn: &SharedByteBufferStream<RawStream>,
        deadline_at: Instant,
        poll_fallback_interval: Duration,
    ) -> Self {
        let readiness = conn.with_inner_ref(RawStream::conn_readiness);
        let fd_ready = match &readiness {
            Some(ConnReadiness::Fd(fd, _)) => match FdReadiness::register(*fd) {
                Ok(r) => Some(r),
                Err(e) => {
                    tracing::debug!("static file: cannot watch the socket on the reactor: {e}");
                    None
                }
            },
            _ => None,
        };
        // Without a reactor registration the kernel-fd transport falls back to
        // timer-paced retries, like a transport that reports no readiness.
        let readiness = match (&readiness, &fd_ready) {
            (Some(ConnReadiness::Fd(..)), None) => None,
            _ => readiness,
        };
        Self {
            readiness,
            fd_ready,
            deadline: timer::sleep_until(deadline_at),
            deadline_at,
            poll_fallback_interval,
            fallback: None,
        }
    }

    /// Wait until the connection is writable (or failed, so the next write sees
    /// why). Fails with `TimedOut` once the response deadline passes.
    async fn writable(&mut self) -> io::Result<()> {
        poll_fn(|cx| self.poll_writable(cx)).await
    }

    fn poll_writable(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if Instant::now() >= self.deadline_at || Pin::new(&mut self.deadline).poll(cx).is_ready() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the client did not read the response within the write timeout",
            )));
        }
        if let Some(readiness) = &self.readiness {
            // Register before the readiness probe, so an edge between the
            // two still wakes this task.
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
                    fd_ready.register_waker(cx.waker());
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
            // No readiness source: park on the timer for one interval,
            // then let the caller retry its write.
            let interval = self.poll_fallback_interval;
            let fallback = self
                .fallback
                .get_or_insert_with(|| timer::sleep_for(interval));
            if Pin::new(fallback).poll(cx).is_ready() {
                self.fallback = None;
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }
    }
}
