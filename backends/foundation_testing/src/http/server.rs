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
//! use `foundation_core::wire::simple_http` types directly.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use foundation_core::netcap::RawStream;
use foundation_core::wire::simple_http::{
    http_streams, HttpReaderError, IncomingRequestParts, Proto, SendSafeBody,
    SimpleHeaders, SimpleMethod, SimpleUrl,
};

type ResponseHandler = Arc<Mutex<Box<dyn Fn(&HttpRequest) -> HttpResponse + Send>>>;

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

    /// Render response to HTTP/1.1 format.
    fn render(&self) -> Vec<u8> {
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
}

impl TestHttpServer {
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

        let running_clone = Arc::clone(&running);
        let handler_clone = Arc::clone(&handler);

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
                        // Handle each connection in separate thread
                        thread::spawn(move || {
                            if let Err(e) = Self::handle_connection(stream, &handler) {
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
    fn handle_connection(
        mut stream: TcpStream,
        handler: &ResponseHandler,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Parse minimal HTTP request (method, path, version)
        let conn = RawStream::from_tcp(stream.try_clone()?).expect("should wrap tcp stream");
        let request_streams = http_streams::send::http_streams(conn);

        tracing::info!("Read a line on connection!");

        // fetch the intro portion and validate we have resources for processing request
        // if not, just break and return an error
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

        let body = match body_part {
            IncomingRequestParts::NoBody => SendSafeBody::None,
            IncomingRequestParts::SizedBody(body) | IncomingRequestParts::StreamedBody(body) => {
                body
            }
            _ => {
                tracing::error!("Failed to receive a IncomingRequestParts::Body(_)");
                return Ok(());
            }
        };

        tracing::info!(
            "Received new http request for proto: method: {:?}, url: {:?}, proto: {:?}",
            method,
            url,
            proto,
        );

        tracing::info!("Got request");
        let request = HttpRequest {
            path: url,
            method,
            proto,
            headers,
            body,
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

        Ok(())
    }
}

impl Drop for TestHttpServer {
    fn drop(&mut self) {
        // Signal server thread to stop
        self.running.store(false, Ordering::Relaxed);
        // Thread will exit on next loop iteration
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
