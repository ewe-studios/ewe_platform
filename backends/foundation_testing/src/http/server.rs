//! Test HTTP server implementation.
//!
//! WHY: Provides real HTTP server for integration tests without external dependencies.
//! Built on stdlib TCP with hand-crafted HTTP responses for simplicity.
//!
//! WHAT: `TestHttpServer` that listens on localhost, accepts requests, and sends responses.
//!
//! HOW: Uses stdlib's `TcpListener` and threading with manually crafted HTTP/1.1 responses.
//! Simple implementation suitable for basic HTTP client testing.
//!
//! NOTE: This is a simplified test server. For production HTTP parsing/rendering,
//! use `foundation_netio::shared::http` types directly.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::{
    HTTPStreams, HttpReaderError, IncomingRequestParts, Proto, SendSafeBody, SimpleHeader,
    SimpleHeaders, SimpleMethod, SimpleUrl,
};

/// How long one read of a request (head or body) may wait for bytes.
///
/// A request head that stalls this long ends the connection with an error; the
/// body reader retries a bounded number of timed-out reads before it gives up.
pub(crate) const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// How long the idle wait sleeps between checks for the next request.
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(1);

/// Wait until the next request starts arriving on an idle keep-alive
/// connection whose socket is non-blocking.
///
/// WHY: the request reader treats `WouldBlock` as a failed read. Parsing a
/// request off a non-blocking socket before its bytes are there ends the
/// connection without an answer, and reading a body that is still in flight
/// gave the handler an empty or truncated body (the body reader gives up after
/// a bounded number of immediate `WouldBlock` retries, and the error was
/// dropped). The socket stays non-blocking only while idle, so the handler
/// can notice the server stopping.
///
/// Returns `Ok(true)` once request bytes are buffered, `Ok(false)` when the
/// peer closed the connection or the server stopped.
fn wait_for_request(
    source: &mut SharedByteBufferStream<RawStream>,
    running: &AtomicBool,
) -> std::io::Result<bool> {
    loop {
        if !running.load(Ordering::Relaxed) {
            return Ok(false);
        }
        match source.probe() {
            Ok(0) => return Ok(false),
            Ok(_) => return Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(IDLE_POLL_INTERVAL);
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err),
        }
    }
}

/// Switch a connection socket between the idle (non-blocking) and the
/// request-reading (blocking, bounded by `timeout`) modes.
///
/// The read half is a `try_clone` of the same socket, so the mode set here
/// applies to both handles.
pub(crate) fn set_request_read_mode(
    socket: &TcpStream,
    timeout: Option<Duration>,
) -> std::io::Result<()> {
    match timeout {
        Some(timeout) => {
            socket.set_nonblocking(false)?;
            socket.set_read_timeout(Some(timeout))
        }
        None => {
            socket.set_read_timeout(None)?;
            socket.set_nonblocking(true)
        }
    }
}

/// Whether the request head asks for an `Expect: 100-continue` handshake.
pub(crate) fn expects_continue(headers: &SimpleHeaders) -> bool {
    headers.get(&SimpleHeader::EXPECT).is_some_and(|values| {
        values
            .iter()
            .any(|v| v.eq_ignore_ascii_case("100-continue"))
    })
}

type ResponseHandler = Arc<Mutex<Box<dyn Fn(&HttpRequest) -> HttpResponse + Send>>>;

/// Handler that can return both an interim (1xx) and final response.
type InterimResponseHandler =
    Arc<Mutex<Box<dyn Fn(&HttpRequest) -> (Option<HttpResponse>, HttpResponse) + Send>>>;

/// Simple HTTP request representation for testing.
#[derive(Debug)]
pub struct HttpRequest {
    /// HTTP method (GET, POST, etc.)
    pub method: SimpleMethod,
    /// Request path (e.g., "/test")
    pub path: SimpleUrl,
    /// HTTP version (e.g., "HTTP/1.1")
    pub proto: Proto,
    /// Request headers
    pub headers: SimpleHeaders,
    /// Body of the request
    pub body: SendSafeBody,
}

/// Simple HTTP response representation for testing.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// Status code (e.g., 200)
    pub status: u16,
    /// Status text (e.g., "OK")
    pub status_text: String,
    /// Response headers
    pub headers: Vec<(String, String)>,
    /// Response body
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Create 200 OK response with body.
    #[must_use]
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        let body_bytes = body.into();
        Self {
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![
                ("Content-Type".to_string(), "text/plain".to_string()),
                ("Content-Length".to_string(), body_bytes.len().to_string()),
            ],
            body: body_bytes,
        }
    }

    /// Create 302 redirect response.
    #[must_use]
    pub fn redirect(location: &str) -> Self {
        Self {
            status: 302,
            status_text: "Found".to_string(),
            headers: vec![
                ("Location".to_string(), location.to_string()),
                ("Content-Length".to_string(), "0".to_string()),
            ],
            body: Vec::new(),
        }
    }

    /// Create custom status response.
    #[must_use]
    pub fn status(code: u16, text: &str) -> Self {
        Self {
            status: code,
            status_text: text.to_string(),
            headers: vec![("Content-Length".to_string(), "0".to_string())],
            body: Vec::new(),
        }
    }

    /// Create 100 Continue interim response.
    #[must_use]
    pub fn continue_response() -> Self {
        Self {
            status: 100,
            status_text: "Continue".to_string(),
            headers: vec![],
            body: Vec::new(),
        }
    }

    /// Render response to HTTP/1.1 format.
    pub(crate) fn render(&self) -> Vec<u8> {
        let mut response = format!("HTTP/1.1 {} {}\r\n", self.status, self.status_text);

        for (key, value) in &self.headers {
            response.push_str(&format!("{key}: {value}\r\n"));
        }

        response.push_str("\r\n");

        let mut bytes = response.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

/// Test HTTP server for integration testing.
///
/// # Purpose (WHY)
///
/// Provides a real HTTP server for testing HTTP clients without external dependencies.
/// Uses stdlib TCP with manually crafted HTTP responses for simplicity.
///
/// # What it does
///
/// Starts a local HTTP server on a random port, accepts incoming requests, and responds
/// with configurable responses. Runs in background thread to not block test execution.
///
/// # Examples
///
/// ```rust
/// use foundation_testing::http::TestHttpServer;
///
/// let server = TestHttpServer::start();
///
/// // Use server.url() in HTTP client tests
/// // let response = some_http_client.get(&server.url("/test")).unwrap();
/// // assert_eq!(response.status(), 200);
///
/// // Server automatically stops when dropped
/// ```
pub struct TestHttpServer {
    addr: String,
    _handle: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
    _handler: ResponseHandler,
    /// When true, the server closes the TCP connection after sending a response.
    /// Useful for testing SSE clients or HTTP/1.0-style servers.
    /// Shared via Arc<AtomicBool> so it can be set after the server thread is spawned.
    close_after_response: Arc<AtomicBool>,
    /// When true, the read stream uses blocking I/O with the configured timeout.
    /// Stored as milliseconds for atomic access.
    read_timeout: Arc<AtomicU64>,
}

impl TestHttpServer {
    /// Create a server that sequentially returns provided (status, location/body) for each request.
    /// For 3xx status: 'location' is Location header, for 2xx: body. Repeats last for further requests.
    ///
    /// # Panics
    ///
    /// This function may panic in the following situations:
    /// - If the provided `steps` vector is empty, the handler will attempt to index `handler_steps.len() - 1`
    ///   when selecting the last response, which will underflow and panic.
    /// - If the mutex used to track progress is poisoned, the call to `Mutex::lock().unwrap()` will panic.
    #[must_use]
    pub fn http_chain(steps: Vec<(u16, &str)>) -> Self {
        use std::sync::{Arc, Mutex};
        let step_count = Arc::new(Mutex::new(0usize));
        let handler_steps = steps
            .into_iter()
            .map(|s| (s.0, s.1.to_string()))
            .collect::<Vec<_>>();
        let progress = Arc::clone(&step_count);
        Self::with_response(move |_req| {
            let mut idx = progress.lock().unwrap();
            let step = if *idx < handler_steps.len() {
                *idx
            } else {
                handler_steps.len() - 1
            };
            *idx += 1;
            let (status, val) = &handler_steps[step];
            match *status {
                301 | 302 | 303 | 307 | 308 => HttpResponse {
                    status: *status,
                    status_text: "Redirect".to_string(),
                    headers: vec![
                        ("Location".to_string(), val.clone()),
                        ("Content-Length".to_string(), "0".to_string()),
                    ],
                    body: Vec::new(),
                },
                200 | 201 | 204 => HttpResponse::ok(val.as_bytes()),
                code => HttpResponse::status(code, val),
            }
        })
    }

    /// Set whether the server should close the TCP connection after sending a response.
    ///
    /// WHY: SSE tests need to verify client behavior when the server closes the connection
    /// after delivering a response. HTTP/1.1 defaults to keep-alive, so this flag lets
    /// tests mimic servers that close after response delivery.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use foundation_testing::http::{HttpResponse, TestHttpServer};
    ///
    /// let server = TestHttpServer::with_response(|_req| HttpResponse::ok(b"data"))
    ///     .close_after_response(true);
    /// ```
    #[must_use]
    pub fn close_after_response(self, close: bool) -> Self {
        self.close_after_response.store(close, Ordering::Relaxed);
        self
    }

    /// Set whether the server's read stream should use blocking I/O with a timeout.
    ///
    /// WHY: by default a connection waits for each request without blocking
    /// (so it notices the server stopping) and then reads the request in
    /// blocking mode bounded by a fixed per-read timeout. Some tests need an
    /// idle keep-alive connection to time out instead, after a known delay.
    ///
    /// # Arguments
    ///
    /// * `timeout` - When `Some`, every read on the connection blocks for at
    ///   most this long, and an idle connection ends when it expires.
    ///   When `None`, keeps the default behavior.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::time::Duration;
    ///
    /// use foundation_testing::http::{HttpResponse, TestHttpServer};
    ///
    /// // Blocking read with 5s timeout
    /// let server = TestHttpServer::with_response(|_req| HttpResponse::ok(b"data"))
    ///     .blocking_read(Some(Duration::from_secs(5)));
    ///
    /// // Default: wait for each request, then read it in full
    /// let server = TestHttpServer::with_response(|_req| HttpResponse::ok(b"data"))
    ///     .blocking_read(None);
    /// ```
    #[must_use]
    pub fn blocking_read(self, timeout: Option<std::time::Duration>) -> Self {
        let ms = timeout.map_or(0, |d| d.as_millis() as u64);
        self.read_timeout.store(ms, Ordering::Relaxed);
        self
    }

    /// Start a new test HTTP server on random port.
    ///
    /// # Returns
    ///
    /// A running `TestHttpServer` that will respond with 200 OK to all requests.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use foundation_testing::http::TestHttpServer;
    ///
    /// let server = TestHttpServer::start();
    /// tracing::info!("Server running at: {}", server.url("/"));
    /// ```
    #[must_use]
    pub fn start() -> Self {
        Self::with_response(|_req| HttpResponse::ok(b"OK"))
    }

    pub fn stop(&mut self) {
        // Signal server thread to stop
        self.running.store(false, Ordering::Relaxed);
        // Thread will exit on next loop iteration
    }

    /// Start server with custom response handler.
    ///
    /// # Purpose (WHY)
    ///
    /// Allows tests to customize server behavior for specific scenarios
    /// (redirects, errors, custom headers, etc.)
    ///
    /// # Arguments
    ///
    /// * `handler` - Function that takes request and returns response
    ///
    /// # Examples
    ///
    /// ```rust
    /// use foundation_testing::http::{TestHttpServer, HttpRequest, HttpResponse};
    ///
    /// let server = TestHttpServer::with_response(|req| {
    ///     if req.path.url == "/redirect" {
    ///         HttpResponse::redirect("/target")
    ///     } else {
    ///         HttpResponse::ok(b"Success")
    ///     }
    /// });
    /// ```
    ///
    /// # Panics
    ///
    /// This function may panic in the following situations:
    /// - If binding to the chosen local address fails (the call to `TcpListener::bind` uses `expect`).
    /// - If retrieving the listener's local address via `local_addr().unwrap()` fails.
    #[must_use]
    pub fn with_response<F>(handler: F) -> Self
    where
        F: Fn(&HttpRequest) -> HttpResponse + Send + 'static,
    {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("Failed to bind test HTTP server to localhost");
        let addr = format!("http://{}", listener.local_addr().unwrap());

        let running = Arc::new(AtomicBool::new(true));
        let handler = Arc::new(Mutex::new(
            Box::new(handler) as Box<dyn Fn(&HttpRequest) -> HttpResponse + Send>
        ));
        let close_after_response = Arc::new(AtomicBool::new(false));
        let read_timeout = Arc::new(AtomicU64::new(0));

        let running_clone = Arc::clone(&running);

        let handler_clone = Arc::clone(&handler);
        let close_clone = Arc::clone(&close_after_response);
        let read_timeout_clone = Arc::clone(&read_timeout);

        let handle = thread::spawn(move || {
            // Set non-blocking so we can check running flag
            listener
                .set_nonblocking(true)
                .expect("Failed to set non-blocking");

            while running_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, sock_addr)) => {
                        tracing::info!("Got a client connection: {sock_addr:?}");
                        let handler = Arc::clone(&handler_clone);
                        let kill_signal = Arc::clone(&running_clone);
                        let should_close = Arc::clone(&close_clone);
                        let read_timeout = Arc::clone(&read_timeout_clone);
                        // Handle each connection in separate thread
                        thread::spawn(move || {
                            if let Err(e) = Self::handle_connection(
                                stream,
                                &handler,
                                kill_signal.clone(),
                                &should_close,
                                &read_timeout,
                            ) {
                                tracing::info!("TestHttpServer connection error: {e}");
                            }
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No connection available, sleep briefly and check again
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => {
                        tracing::info!("TestHttpServer accept error: {e}");
                        break;
                    }
                }
            }
        });

        Self {
            addr,
            _handle: Some(handle),
            running,
            _handler: handler,
            close_after_response,
            read_timeout,
        }
    }

    /// Start server with handler that can send interim (1xx) and final responses.
    ///
    /// # Purpose (WHY)
    ///
    /// Allows testing scenarios where server sends 100 Continue before the final response,
    /// such as redirect-after-continue edge cases.
    ///
    /// # Arguments
    ///
    /// * `handler` - Function that takes request and returns (optional interim, final) response
    ///
    /// # Examples
    ///
    /// ```rust
    /// use foundation_testing::http::{TestHttpServer, HttpRequest, HttpResponse};
    ///
    /// let server = TestHttpServer::with_interim_response(|req| {
    ///     // Send 100 Continue first, then final response
    ///     (Some(HttpResponse::continue_response()), HttpResponse::redirect("/target"))
    /// });
    /// ```
    #[must_use]
    pub fn with_interim_response<F>(handler: F) -> Self
    where
        F: Fn(&HttpRequest) -> (Option<HttpResponse>, HttpResponse) + Send + 'static,
    {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("Failed to bind test HTTP server to localhost");
        let addr = format!("http://{}", listener.local_addr().unwrap());

        let running = Arc::new(AtomicBool::new(true));
        let handler = Arc::new(Mutex::new(Box::new(handler)
            as Box<dyn Fn(&HttpRequest) -> (Option<HttpResponse>, HttpResponse) + Send>));
        let close_after_response = Arc::new(AtomicBool::new(false));
        let read_timeout = Arc::new(AtomicU64::new(0));

        let running_clone = Arc::clone(&running);
        let handler_clone = Arc::clone(&handler);
        let close_clone = Arc::clone(&close_after_response);
        let read_timeout_clone = Arc::clone(&read_timeout);

        let handle = thread::spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("Failed to set non-blocking");

            while running_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, sock_addr)) => {
                        tracing::info!("Got a client connection: {sock_addr:?}");
                        let handler = Arc::clone(&handler_clone);
                        let should_close = Arc::clone(&close_clone);
                        let read_timeout = Arc::clone(&read_timeout_clone);
                        thread::spawn(move || {
                            if let Err(e) = Self::handle_connection_with_interim(
                                stream,
                                &handler,
                                &should_close,
                                &read_timeout,
                            ) {
                                tracing::info!("TestHttpServer connection error: {e}");
                            }
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => {
                        tracing::info!("TestHttpServer accept error: {e}");
                        break;
                    }
                }
            }
        });

        Self {
            addr,
            _handle: Some(handle),
            running,
            _handler: Arc::new(Mutex::new(Box::new(|_| HttpResponse::ok(b"")))),
            close_after_response,
            read_timeout,
        }
    }

    /// Get full URL for a path on this test server.
    ///
    /// # Arguments
    ///
    /// * `path` - Path starting with / (e.g., "/test", "/api/users")
    ///
    /// # Returns
    ///
    /// Full URL string (e.g., "<http://127.0.0.1:54321/test>")
    ///
    /// # Examples
    ///
    /// ```rust
    /// use foundation_testing::http::TestHttpServer;
    ///
    /// let server = TestHttpServer::start();
    /// assert!(server.url("/test").starts_with("http://127.0.0.1:"));
    /// assert!(server.url("/test").ends_with("/test"));
    /// ```
    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.addr, path)
    }

    /// Get base URL of this test server.
    ///
    /// # Returns
    ///
    /// Base URL without path (e.g., "<http://127.0.0.1:54321>")
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.addr
    }

    /// Handle a single HTTP connection.
    ///
    /// WHY: Processes incoming HTTP request and sends response.
    #[tracing::instrument(skip(stream, handler))]
    fn handle_connection(
        mut stream: TcpStream,
        handler: &ResponseHandler,
        running: Arc<AtomicBool>,
        close_after_response: &Arc<AtomicBool>,
        read_timeout_ms: &Arc<AtomicU64>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Handle multiple requests per connection (HTTP keep-alive)
        // Loop until connection closes or max requests reached
        const MAX_REQUESTS_PER_CONN: usize = 10;

        // Clone the stream for reading — keep original for writing responses.
        // The reader consumes bytes; the writer needs the original socket.
        let read_stream: TcpStream = stream.try_clone()?;

        // A configured `blocking_read` timeout applies to every read for the
        // connection's whole life. Otherwise the socket is non-blocking while
        // idle (so the handler notices the server stopping) and blocking,
        // bounded by `REQUEST_READ_TIMEOUT`, while a request is being read:
        // the request reader treats `WouldBlock` as a failed read, so a
        // request must never be parsed off a non-blocking socket.
        let timeout_ms = read_timeout_ms.load(Ordering::Relaxed);
        let configured_timeout = (timeout_ms > 0).then(|| Duration::from_millis(timeout_ms));
        set_request_read_mode(&read_stream, configured_timeout)?;

        let conn = RawStream::from_tcp(read_stream)?;
        let mut source = SharedByteBufferStream::rwrite(conn);
        let request_streams = HTTPStreams::new(source.clone());

        for _req_num in 0..MAX_REQUESTS_PER_CONN {
            if configured_timeout.is_none() {
                match wait_for_request(&mut source, &running) {
                    Ok(true) => set_request_read_mode(&stream, Some(REQUEST_READ_TIMEOUT))?,
                    Ok(false) => {
                        tracing::debug!(
                            "[TestHTTPServer] Peer closed the connection or the server stopped"
                        );
                        break;
                    }
                    Err(err) => {
                        tracing::warn!(
                            "[TestHTTPServer] Waiting for the next request failed: {err}"
                        );
                        return Err(err.into());
                    }
                }
            } else if !running.load(Ordering::Relaxed) {
                tracing::debug!("[TestHTTPServer] Killing next request reader");
                break;
            }

            let request_reader = request_streams.next_request();
            tracing::debug!("Pulled next request");

            // Answer `Expect: 100-continue` before pulling the body.
            //
            // WHY: our own client sends `Expect: 100-continue` on every request
            // with a body by default (`ClientConfig::expect_continue_enabled`),
            // and then waits for the interim response before writing the body. A
            // server that never sends it gets an empty body and no error — the
            // request looks well-formed and simply has nothing in it.
            //
            // The parts iterator is lazy, so this sits between reading the
            // headers and reading the body: exactly where RFC 9110 §10.1.1 puts
            // it. `with_interim_response` also handles the handshake, but it
            // discards request bodies, so neither constructor could inspect what
            // a POST actually sent until this.
            let mut parts_iter = request_reader
                .into_iter()
                .filter(|item| !matches!(item, Ok(IncomingRequestParts::SKIP)));

            let mut collected: Vec<IncomingRequestParts> = Vec::new();
            let mut read_error: Option<HttpReaderError> = None;
            for item in parts_iter.by_ref() {
                match item {
                    Ok(part) => {
                        let expects_continue = matches!(
                            &part,
                            IncomingRequestParts::Headers(headers) if expects_continue(headers)
                        );
                        collected.push(part);
                        if expects_continue {
                            tracing::debug!(
                                "[TestHTTPServer] Expect: 100-continue — sending interim"
                            );
                            stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
                            stream.flush()?;
                        }
                    }
                    Err(e) => {
                        read_error = Some(e);
                        break;
                    }
                }
            }
            let parts: Result<Vec<IncomingRequestParts>, HttpReaderError> = match read_error {
                Some(e) => Err(e),
                None => Ok(collected),
            };

            tracing::debug!("Collected all parts of request");
            if let Err(part_err) = parts {
                tracing::debug!("Failed to read request parts: {part_err:?}, ending connection");
                break;
            }

            tracing::debug!("Unwrap into request parts");
            let mut request_parts = parts.unwrap();
            if request_parts.len() != 3 {
                tracing::debug!(
                    "Unexpected request parts count (expected 3): {:?}",
                    &request_parts
                );
                break;
            }

            let body_part = request_parts.pop().unwrap();
            let headers_part = request_parts.pop().unwrap();
            let intros_part = request_parts.pop().unwrap();

            tracing::debug!("Deconstruct request parts");

            let IncomingRequestParts::Intro(method, url, proto) = intros_part else {
                tracing::debug!("Failed to receive a IncomingRequestParts::Intro(_, _, _)");
                break;
            };

            let IncomingRequestParts::Headers(headers) = headers_part else {
                tracing::debug!("Failed to receive a IncomingRequestParts::Headers(_)");
                break;
            };

            let body_part = match body_part {
                IncomingRequestParts::NoBody => SendSafeBody::None,
                IncomingRequestParts::SizedBody(body)
                | IncomingRequestParts::StreamedBody(body) => body,
                _other => {
                    tracing::debug!("Failed to receive a IncomingRequestParts::Body(_)");
                    break;
                }
            };

            tracing::trace!("[HTTP TEST SERVER] Read the body of request");
            // The whole body is read before the handler runs: a
            // `Content-Length` body to exactly that length, a chunked body to
            // its last chunk. A body that cannot be read in full ends the
            // connection with an error rather than reaching the handler empty
            // or truncated.
            let body = match try_collect_bytes(body_part) {
                Ok(body) => body,
                Err(err) => {
                    tracing::warn!("Failed to read the request body, ending connection: {err}");
                    return Err(err);
                }
            };
            if configured_timeout.is_none() {
                set_request_read_mode(&stream, None)?;
            }

            tracing::info!("Got request");
            let request = HttpRequest {
                path: url,
                method,
                proto,
                headers,
                body: SendSafeBody::Bytes(body),
            };

            // Call user's handler to get response
            let response = {
                let handler_guard = handler.lock().unwrap();
                handler_guard(&request)
            };

            // Send response
            tracing::info!("render response");
            let rendered = response.render();
            stream.write_all(&rendered)?;
            stream.flush()?;
            tracing::info!("flush response");

            // Check if Connection: close was requested or configured
            // HTTP/1.1 defaults to keep-alive, so we only close if explicitly requested
            let should_close = close_after_response.load(Ordering::Relaxed)
                || response.headers.iter().any(|(k, v)| {
                    k.eq_ignore_ascii_case("connection") && v.eq_ignore_ascii_case("close")
                });

            if should_close {
                tracing::debug!("Connection: close requested, closing connection");
                break;
            }
        }

        Ok(())
    }

    /// Handle a single HTTP connection with interim (1xx) response support.
    ///
    /// WHY: Processes incoming HTTP request and sends interim + final responses.
    fn handle_connection_with_interim(
        mut stream: TcpStream,
        handler: &InterimResponseHandler,
        close_after_response: &Arc<AtomicBool>,
        read_timeout_ms: &Arc<AtomicU64>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let timeout_ms = read_timeout_ms.load(Ordering::Relaxed);
        let read_stream: TcpStream = stream.try_clone()?;
        // One request per connection, read in blocking mode (the request
        // reader treats `WouldBlock` as a failed read), with every read
        // bounded so a silent client cannot pin the connection thread.
        let read_timeout = if timeout_ms > 0 {
            Duration::from_millis(timeout_ms)
        } else {
            REQUEST_READ_TIMEOUT
        };
        set_request_read_mode(&read_stream, Some(read_timeout))?;
        // Parse minimal HTTP request (method, path, version)
        let conn = RawStream::from_tcp(read_stream)?;
        let request_streams = HTTPStreams::new(SharedByteBufferStream::rwrite(conn));

        tracing::info!("Read a line on connection!");

        // fetch the intro portion and validate we have resources for processing request
        let request_reader = request_streams.next_request();
        tracing::debug!("Pulled next request");

        let parts: Result<Vec<IncomingRequestParts>, HttpReaderError> = request_reader
            .into_iter()
            .filter(|item| match item {
                Ok(IncomingRequestParts::SKIP) => false,
                Ok(_) | Err(_) => true,
            })
            .collect();

        tracing::debug!("Collected all parts of request");
        if let Err(part_err) = parts {
            tracing::error!("Failed to read requests from reader due to: {:?}", part_err);
            return Ok(());
        }

        tracing::debug!("Unwrap into request parts");
        let mut request_parts = parts.unwrap();
        if request_parts.len() != 3 {
            tracing::error!(
                "Failed to receive expected request parts of 3: {:?}",
                &request_parts
            );
            return Ok(());
        }

        let body_part = request_parts.pop().unwrap();
        let headers_part = request_parts.pop().unwrap();
        let intros_part = request_parts.pop().unwrap();

        tracing::debug!("Deconstruct request parts");

        let IncomingRequestParts::Intro(method, url, proto) = intros_part else {
            tracing::error!("Failed to receive a IncomingRequestParts::Intro(_, _, _)");
            return Ok(());
        };

        let IncomingRequestParts::Headers(headers) = headers_part else {
            tracing::error!("Failed to receive a IncomingRequestParts::Headers(_)");
            return Ok(());
        };

        tracing::debug!("Reviewing body part: {:?}", body_part);

        let body_part = match body_part {
            IncomingRequestParts::NoBody => SendSafeBody::None,
            IncomingRequestParts::SizedBody(body) | IncomingRequestParts::StreamedBody(body) => {
                body
            }
            _ => {
                tracing::error!("Failed to receive a IncomingRequestParts::Body(_)");
                return Ok(());
            }
        };

        // Call handler once to get interim + final response BEFORE reading body.
        // WHY: Clients sending Expect: 100-continue wait for the server to
        // respond before sending the body. If we read body first, we deadlock.
        // We cache the final response so it's sent after body is consumed.
        let (interim_response, final_response) = {
            let request_for_handler = HttpRequest {
                path: url.clone(),
                method: method.clone(),
                proto: proto.clone(),
                headers: headers.clone(),
                body: SendSafeBody::None,
            };
            let handler_guard = handler.lock().unwrap();
            handler_guard(&request_for_handler)
        };

        // Send interim response if present to unblock Expect: 100-continue
        if let Some(interim) = interim_response {
            tracing::info!("render interim response");
            let rendered = interim.render();
            stream.write_all(&rendered)?;
            stream.flush()?;
            tracing::info!("flush interim response");
        }

        // NOW read the body (client will send it after receiving 100 Continue)
        tracing::trace!("[HTTP TEST SERVER] Read the body of request");
        // The socket is blocking, so the read waits for the body. The body is
        // consumed to keep the connection in sync; its content isn't used,
        // but a failed read still ends the connection with an error.
        if let Err(err) = try_collect_bytes(body_part) {
            tracing::warn!("Failed to read the request body, ending connection: {err}");
            return Err(err);
        }

        tracing::info!(
            "Received new http request for proto: method: {:?}, url: {:?}, proto: {:?}",
            method,
            url,
            proto,
        );

        // Send the cached final response (determined before body was read)
        tracing::info!("render final response");
        let rendered = final_response.render();
        stream.write_all(&rendered)?;
        stream.flush()?;
        tracing::info!("flush final response");

        // Close the connection if configured to do so after response
        if close_after_response.load(Ordering::Relaxed) {
            tracing::debug!("Closing connection after response delivery");
            drop(stream);
        }

        Ok(())
    }
}

impl Drop for TestHttpServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WHY: Verify TestHttpServer can start and provides valid URL
    /// WHAT: Tests basic construction and URL generation
    #[test]
    fn test_server_start() {
        let server = TestHttpServer::start();
        assert!(server.base_url().starts_with("http://127.0.0.1:"));
        assert!(server.url("/test").ends_with("/test"));
    }

    /// WHY: Verify custom response handler works
    /// WHAT: Tests that custom handler is called
    #[test]
    fn test_custom_response() {
        let server = TestHttpServer::with_response(|req| {
            if req.path.url.as_str() == "/redirect" {
                HttpResponse::redirect("/target")
            } else {
                HttpResponse::status(201, "Created")
            }
        });

        // Just verify server starts (actual HTTP testing requires HTTP client)
        assert!(server.base_url().starts_with("http://"));
    }

    /// WHY: Verify HttpResponse::ok creates proper response
    /// WHAT: Tests response construction
    #[test]
    fn test_http_response_ok() {
        let response = HttpResponse::ok(b"test");
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"test");
    }

    /// WHY: Verify HttpResponse::redirect creates proper response
    /// WHAT: Tests redirect response construction
    #[test]
    fn test_http_response_redirect() {
        let response = HttpResponse::redirect("/new-location");
        assert_eq!(response.status, 302);
        assert!(response
            .headers
            .iter()
            .any(|(k, v)| k == "Location" && v == "/new-location"));
    }

    /// WHY: Verify server stops cleanly when dropped
    /// WHAT: Tests Drop implementation
    #[test]
    fn test_server_drop() {
        {
            let _server = TestHttpServer::start();
            // Server running
        }
        // Server should have stopped after drop
        // No assertion needed - test passes if no panic/hang
    }

    /// One client action in a scripted exchange with the server.
    enum Step {
        /// Write these bytes to the socket.
        Write(&'static [u8]),
        /// Wait before the next step, so the server sees a partial request.
        Pause(Duration),
        /// Read and check the `100 Continue` interim response.
        ReadInterim,
    }

    /// Runs `steps` against a server that records every request body it
    /// hands to its handler, then reads until the server closes the
    /// connection. A request to `/last` is answered with `Connection: close`.
    /// Returns the recorded bodies (in order) and everything the server sent.
    fn exchange(steps: &[Step]) -> (Vec<Vec<u8>>, String) {
        use std::io::Read;

        let seen = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let recorder = Arc::clone(&seen);
        let server = TestHttpServer::with_response(move |req| {
            let captured = match &req.body {
                SendSafeBody::Bytes(bytes) => bytes.clone(),
                other => panic!("the server must hand handlers a byte body, got {other:?}"),
            };
            recorder.lock().unwrap().push(captured);
            let mut response = HttpResponse::ok(b"done");
            if req.path.url == "/last" {
                response
                    .headers
                    .push(("Connection".to_string(), "close".to_string()));
            }
            response
        });

        let addr = server.base_url().trim_start_matches("http://").to_string();
        let mut client = TcpStream::connect(&addr).expect("connect to test server");
        client
            .set_read_timeout(Some(Duration::from_secs(30)))
            .expect("bound the client's reads");
        for step in steps {
            match step {
                Step::Write(bytes) => {
                    client.write_all(bytes).expect("write to the server");
                    client.flush().expect("flush to the server");
                }
                Step::Pause(delay) => thread::sleep(*delay),
                Step::ReadInterim => {
                    let interim = b"HTTP/1.1 100 Continue\r\n\r\n";
                    let mut buf = vec![0u8; interim.len()];
                    client
                        .read_exact(&mut buf)
                        .expect("read the interim response");
                    assert_eq!(buf, interim, "the server must answer Expect: 100-continue");
                }
            }
        }

        let mut response = String::new();
        client
            .read_to_string(&mut response)
            .expect("read the server's responses until it closes");
        let bodies = std::mem::take(&mut *seen.lock().unwrap());
        (bodies, response)
    }

    /// Number of complete `200 OK` responses in `raw`.
    fn ok_responses(raw: &str) -> usize {
        raw.matches("HTTP/1.1 200 OK\r\n").count()
    }

    /// WHY: the server read bodies from a non-blocking socket with a bounded
    /// number of immediate retries, so a body that arrived a moment after the
    /// headers reached the handler empty (the flaky body capture in
    /// foundation_ai's provider_fault_injection_tests).
    /// WHAT: a `Content-Length` body sent 200ms after the headers is captured
    /// in full.
    #[test]
    fn late_body_is_captured_in_full() {
        let (bodies, response) = exchange(&[
            Step::Write(
                b"POST /last HTTP/1.1\r\nHost: test\r\nContent-Type: application/octet-stream\r\nContent-Length: 15\r\n\r\n",
            ),
            Step::Pause(Duration::from_millis(200)),
            Step::Write(b"late body bytes"),
        ]);
        assert_eq!(bodies, vec![b"late body bytes".to_vec()]);
        assert_eq!(ok_responses(&response), 1, "response: {response}");
    }

    /// WHY: a `Content-Length` body that arrives in pieces must be read to its
    /// declared length, not to the end of the first read.
    /// WHAT: a body written in two halves 100ms apart is captured in full.
    #[test]
    fn body_split_across_writes_is_captured_in_full() {
        let (bodies, response) = exchange(&[
            Step::Write(
                b"POST /last HTTP/1.1\r\nHost: test\r\nContent-Length: 23\r\n\r\nfirst half, ",
            ),
            Step::Pause(Duration::from_millis(100)),
            Step::Write(b"second half"),
        ]);
        assert_eq!(bodies, vec![b"first half, second half".to_vec()]);
        assert_eq!(ok_responses(&response), 1, "response: {response}");
    }

    /// WHY: our own client sends `Expect: 100-continue` and writes the body
    /// only after the interim response, so the body can never be there when
    /// the headers are parsed.
    /// WHAT: the body sent after `100 Continue` is captured in full.
    #[test]
    fn body_after_expect_continue_is_captured_in_full() {
        let (bodies, response) = exchange(&[
            Step::Write(
                b"POST /last HTTP/1.1\r\nHost: test\r\nContent-Type: application/json\r\nContent-Length: 26\r\nExpect: 100-continue\r\n\r\n",
            ),
            Step::ReadInterim,
            Step::Pause(Duration::from_millis(50)),
            Step::Write(b"{\"input\":\"after continue\"}"),
        ]);
        assert_eq!(bodies, vec![b"{\"input\":\"after continue\"}".to_vec()]);
        assert_eq!(ok_responses(&response), 1, "response: {response}");
    }

    /// WHY: chunked bodies have no length up front; the server must read to
    /// the last chunk even when the chunks arrive over time.
    /// WHAT: a chunked body sent as three writes is captured in full.
    #[test]
    fn chunked_body_arriving_over_time_is_captured_in_full() {
        let (bodies, response) = exchange(&[
            Step::Write(
                b"POST /last HTTP/1.1\r\nHost: test\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n",
            ),
            Step::Pause(Duration::from_millis(100)),
            Step::Write(b"6\r\n world\r\n"),
            Step::Pause(Duration::from_millis(100)),
            Step::Write(b"0\r\n\r\n"),
        ]);
        assert_eq!(bodies, vec![b"hello world".to_vec()]);
        assert_eq!(ok_responses(&response), 1, "response: {response}");
    }

    /// WHY: parsing a request off a non-blocking socket before its bytes
    /// arrived ended the connection without an answer.
    /// WHAT: a request whose head arrives late, and split in two, is answered.
    #[test]
    fn late_and_split_request_head_is_answered() {
        let (bodies, response) = exchange(&[
            Step::Pause(Duration::from_millis(200)),
            Step::Write(b"POST /last HTTP/1.1\r\nHost: te"),
            Step::Pause(Duration::from_millis(100)),
            Step::Write(b"st\r\nContent-Length: 4\r\n\r\nbody"),
        ]);
        assert_eq!(bodies, vec![b"body".to_vec()]);
        assert_eq!(ok_responses(&response), 1, "response: {response}");
    }

    /// WHY: an idle keep-alive connection was closed as soon as the next
    /// request was not already buffered, so a client reusing it lost the
    /// request.
    /// WHAT: a second request sent 200ms after the first is answered on the
    /// same connection.
    #[test]
    fn keep_alive_connection_serves_a_later_request() {
        let (bodies, response) = exchange(&[
            Step::Write(b"POST /first HTTP/1.1\r\nHost: test\r\nContent-Length: 3\r\n\r\none"),
            Step::Pause(Duration::from_millis(200)),
            Step::Write(b"POST /last HTTP/1.1\r\nHost: test\r\nContent-Length: 3\r\n\r\ntwo"),
        ]);
        assert_eq!(bodies, vec![b"one".to_vec(), b"two".to_vec()]);
        assert_eq!(ok_responses(&response), 2, "response: {response}");
    }
}
