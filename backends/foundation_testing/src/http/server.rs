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

use foundation_netio::netcap::RawStream;
use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;
use foundation_netio::shared::http::{
    http_streams, HttpReaderError, IncomingRequestParts, Proto, SendSafeBody, SimpleHeaders,
    SimpleMethod, SimpleUrl,
};

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
    /// WHY: The HTTP reader treats WouldBlock from non-blocking sockets as a fatal
    /// error. Tests with precise client timing (e.g., connection pooling tests)
    /// need blocking read to avoid premature connection handler exit.
    ///
    /// # Arguments
    ///
    /// * `timeout` - When `Some`, enables blocking read with the given timeout.
    ///   When `None`, keeps the default non-blocking behavior.
    ///
    /// # Examples
    ///
    /// ```rust
    /// // Blocking read with 5s timeout
    /// let server = TestHttpServer::with_response(|_req| HttpResponse::ok(b"data"))
    ///     .blocking_read(Some(Duration::from_secs(5)));
    ///
    /// // Non-blocking (default)
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
    ///     if req.path == "/redirect" {
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
        let read_stream: TcpStream = stream.try_clone().expect("should clone tcp stream");

        let timeout_ms = read_timeout_ms.load(Ordering::Relaxed);
        if timeout_ms > 0 {
            // Blocking read with timeout — HTTP reader treats WouldBlock as fatal,
            // so blocking mode with a timeout prevents premature connection handler exit.
            read_stream
                .set_nonblocking(false)
                .expect("should set blocking on read stream");
            read_stream
                .set_read_timeout(Some(std::time::Duration::from_millis(timeout_ms)))
                .expect("should set read timeout");
        } else {
            // Default: non-blocking so the handler can check the running flag.
            read_stream
                .set_nonblocking(true)
                .expect("should enable non-blocking");
        }

        let conn = RawStream::from_tcp(read_stream).expect("should wrap tcp stream");
        let request_streams = http_streams::send::http_streams(conn);

        let running_clone = Arc::clone(&running);

        for _req_num in 0..MAX_REQUESTS_PER_CONN {
            if !running_clone.load(Ordering::Relaxed) {
                tracing::debug!("[TestHTTPServer] Killing next request reader");
                break;
            }

            // fetch the intro portion and validate we have resources for processing request
            tracing::debug!("[TestHTTPServer] Pulled next request");
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
            let body = collect_bytes_from_send_safe(body_part);

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
        let read_stream: TcpStream = stream.try_clone().expect("should clone tcp stream");
        if timeout_ms > 0 {
            read_stream
                .set_nonblocking(false)
                .expect("should set blocking on read stream");
            read_stream
                .set_read_timeout(Some(std::time::Duration::from_millis(timeout_ms)))
                .expect("should set read timeout");
        }
        // Parse minimal HTTP request (method, path, version)
        let conn = RawStream::from_tcp(read_stream).expect("should wrap tcp stream");
        let request_streams = http_streams::send::http_streams(conn);

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
        let _body = collect_bytes_from_send_safe(body_part);

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
}
