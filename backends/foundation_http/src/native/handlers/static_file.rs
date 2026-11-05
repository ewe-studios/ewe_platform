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
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{SimpleHeader, SimpleIncomingRequest, SimpleMethod};

use crate::native::writer::{
    log_write_failure, render_head, send_not_found, send_server_error, send_text, ResponseWriter,
    WriterConfig,
};
use crate::shared::context::ContextBag;
use crate::shared::serve::{ConnectionResult, Serve, ServeFactory, ServeOutcome};

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
    /// The longest the socket may take no byte of the response. The timer
    /// restarts after every write that takes at least one byte, so a long
    /// download to a slow but live client is not cut; a client that stops
    /// reading is closed. Default 60 s, nginx's `send_timeout` (the time between
    /// two successive writes).
    pub send_timeout: Duration,
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
            send_timeout: Duration::from_secs(60),
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

    /// Set [`Self::send_timeout`].
    #[must_use]
    pub fn with_send_timeout(mut self, timeout: Duration) -> Self {
        self.send_timeout = timeout;
        self
    }

    /// Old name of [`Self::with_send_timeout`]. The setting was a total
    /// deadline; it is now an idle timeout.
    #[must_use]
    #[deprecated(note = "use `with_send_timeout`; the setting is now an idle timeout")]
    pub fn with_write_timeout(self, timeout: Duration) -> Self {
        self.with_send_timeout(timeout)
    }

    fn writer_config(&self) -> WriterConfig {
        WriterConfig::new()
            .with_send_timeout(self.send_timeout)
            .with_poll_fallback_interval(self.poll_fallback_interval)
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
        // Prevent path traversal
        let safe_path = path.trim_start_matches('/');
        if safe_path.contains("..") {
            return send_text(conn, 403, "Forbidden");
        }

        let file_path = self.root.join(safe_path);

        // Verify the resolved path is still under root
        if !file_path.starts_with(&self.root) {
            return send_text(conn, 403, "Forbidden");
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
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let opened = File::open(path).and_then(|file| {
            let meta = file.metadata()?;
            Ok((file, meta))
        });
        let (file, meta) = match opened {
            Ok((file, meta)) if meta.is_file() => (file, meta),
            Ok(_) => {
                return send_not_found(conn);
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return send_not_found(conn);
            }
            Err(e) => {
                tracing::error!("Failed to open static file {path:?}: {e}");
                return send_server_error(conn, None);
            }
        };
        let len = meta.len();

        // The head is small; render it now so a render failure closes the
        // connection before any byte is written.
        let headers = [(SimpleHeader::CONTENT_TYPE, mime_type(path).to_string())];
        let head = match render_head(200, &headers, usize::try_from(len).unwrap_or(usize::MAX)) {
            Ok(head) => head,
            Err(e) => {
                tracing::error!("Failed to render static file head: {e}");
                return ConnectionResult::Close(None);
            }
        };

        let writer = ResponseWriter::new(conn, self.config.writer_config());
        let job = SendFile {
            writer,
            head,
            file,
            len: if head_only { 0 } else { len },
            chunk_size: self.config.chunk_size,
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
    writer: ResponseWriter,
    head: Vec<u8>,
    file: File,
    /// Body bytes to send (0 for a `HEAD` request).
    len: u64,
    chunk_size: usize,
}

impl SendFile {
    async fn run(mut self) -> ServeOutcome {
        match self.send().await {
            Ok(()) => ServeOutcome::Keep,
            Err(e) => {
                log_write_failure(&e);
                ServeOutcome::Close(None)
            }
        }
    }

    async fn send(&mut self) -> io::Result<()> {
        let head = std::mem::take(&mut self.head);
        self.writer.write_all(&head).await?;

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
            self.writer.write_all(&buf[..n]).await?;
            remaining -= n as u64;
        }
        self.writer.flush().await
    }
}
