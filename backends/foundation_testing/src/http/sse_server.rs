//! SSE (Server-Sent Events) test server.
//!
//! WHY: `TestHttpServer` is request-response and always closes cleanly (EOF).
//! SSE clients need a server that can stream events and then drop the connection
//! in controlled ways — clean EOF, abrupt close, or error — to test reconnection,
//! error handling, and event parsing.
//!
//! WHAT: A bare-bones HTTP server that accepts SSE connections and calls a
//! per-connection handler with full control over event delivery and close behavior.
//!
//! HOW: Uses stdlib `TcpListener` with the same HTTP parsing infrastructure as
//! `TestHttpServer`. Each connection is handled in a separate thread.

use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use socket2::SockRef;

use foundation_netio::netcap::RawStream;
use foundation_netio::simple_http::client::shared::body_reader::collect_bytes_from_send_safe;
use foundation_netio::simple_http::shared::{
    http_streams, HttpReaderError, IncomingRequestParts, Proto, SendSafeBody,
};

use crate::http::HttpRequest;

/// Set SO_LINGER=0 on the socket to force TCP RST on close.
///
/// WHY: Without SO_LINGER=0, closing a TcpStream sends FIN (graceful close).
/// With SO_LINGER=0, the kernel discards pending data and sends RST, which
/// causes the peer to see a connection reset error instead of EOF.
fn set_so_linger_zero(stream: &TcpStream) -> io::Result<()> {
    let sock_ref = SockRef::from(stream);
    sock_ref.set_linger(Some(Duration::ZERO))
}

/// How the SSE stream should end after the handler finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SseCloseBehavior {
    /// Write remaining buffered events, then cleanly close (TCP FIN).
    /// Triggers `EventSourceCloseReason::Eof` — no reconnection.
    CleanClose,
    /// Drop the connection without closing properly (no TCP FIN).
    /// Triggers a connection error — reconnects if retries remain.
    AbruptDrop,
    /// Write a partial/corrupted SSE line before dropping.
    /// Triggers a parse error — reconnects if retries remain.
    CorruptData,
}

/// What a per-connection handler returns after delivering events.
#[derive(Debug)]
pub struct SseConnectionResult {
    /// How the connection should be closed.
    pub close: SseCloseBehavior,
}

impl SseConnectionResult {
    /// Cleanly close after delivering all events.
    pub fn clean_close() -> Self {
        Self {
            close: SseCloseBehavior::CleanClose,
        }
    }

    /// Abruptly drop the connection (simulates network failure).
    pub fn abrupt_drop() -> Self {
        Self {
            close: SseCloseBehavior::AbruptDrop,
        }
    }

    /// Write partial data then drop (simulates corrupted stream).
    pub fn corrupt_data() -> Self {
        Self {
            close: SseCloseBehavior::CorruptData,
        }
    }
}

/// Response handler type: receives parsed request, stream writer, and connection count.
type SseResponseHandler = Arc<
    std::sync::Mutex<
        Box<dyn Fn(&HttpRequest, &mut SseStreamWriter, usize) -> SseConnectionResult + Send>,
    >,
>;

/// Writer for SSE events on the raw TCP stream.
///
/// WHY: Provides a typed API for writing SSE events without dealing with
/// low-level byte formatting. Handlers call `event()` to queue events,
/// then the close behavior determines how the connection ends.
pub struct SseStreamWriter {
    stream: TcpStream,
    headers_written: bool,
}

impl SseStreamWriter {
    /// Write an SSE event with the given data.
    ///
    /// Automatically writes HTTP headers on first call.
    pub fn event(&mut self, data: &str) -> io::Result<()> {
        if !self.headers_written {
            self.write_headers()?;
            self.headers_written = true;
        }
        write!(self.stream, "data: {data}\n\n")?;
        self.stream.flush()
    }

    /// Write a custom raw line to the stream (for testing malformed SSE).
    pub fn raw(&mut self, data: &str) -> io::Result<()> {
        if !self.headers_written {
            self.write_headers()?;
            self.headers_written = true;
        }
        write!(self.stream, "{data}")?;
        self.stream.flush()
    }

    /// Write a retry directive.
    pub fn retry(&mut self, ms: u64) -> io::Result<()> {
        if !self.headers_written {
            self.write_headers()?;
            self.headers_written = true;
        }
        write!(self.stream, "retry: {ms}\n\n")?;
        self.stream.flush()
    }

    /// Write an SSE event with a custom event type.
    pub fn event_with_type(&mut self, data: &str, event_type: &str) -> io::Result<()> {
        if !self.headers_written {
            self.write_headers()?;
            self.headers_written = true;
        }
        write!(self.stream, "event: {event_type}\ndata: {data}\n\n")?;
        self.stream.flush()
    }

    /// Write an SSE event with a custom ID.
    pub fn event_with_id(&mut self, data: &str, id: &str) -> io::Result<()> {
        if !self.headers_written {
            self.write_headers()?;
            self.headers_written = true;
        }
        write!(self.stream, "id: {id}\ndata: {data}\n\n")?;
        self.stream.flush()
    }

    fn write_headers(&mut self) -> io::Result<()> {
        write!(
            self.stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\n"
        )?;
        self.stream.flush()
    }
}

/// SSE test server for integration testing.
///
/// # Purpose (WHY)
///
/// Provides a real HTTP server that can stream SSE events and then close the
/// connection in controlled ways (clean EOF, abrupt drop, or corrupted data).
/// This enables testing SSE client reconnection, error handling, and
/// Last-Event-ID resume behavior.
///
/// # What it does
///
/// Starts a local HTTP server on a random port, accepts incoming connections,
/// parses the HTTP request, and calls the per-connection handler. The handler
/// receives the parsed request, an `SseStreamWriter`, and the connection count,
/// and returns an `SseConnectionResult` that determines how the connection ends.
///
/// # Examples
///
/// ```rust
/// use foundation_testing::sse_server::{SseTestServer, SseConnectionResult, SseStreamWriter};
/// use foundation_testing::http::HttpRequest;
///
/// // Server that sends one event then cleanly closes
/// let server = SseTestServer::with_handler(|_req: &HttpRequest, writer: &mut SseStreamWriter, _count: usize| {
///     writer.event(r#"{"msg": "hello"}"#).unwrap();
///     SseConnectionResult::clean_close()
/// });
///
/// // Server that sends one event then abruptly drops (triggers reconnection)
/// let server = SseTestServer::with_handler(|_req: &HttpRequest, writer: &mut SseStreamWriter, _count: usize| {
///     writer.event(r#"{"msg": "hello"}"#).unwrap();
///     SseConnectionResult::abrupt_drop()
/// });
/// ```
pub struct SseTestServer {
    addr: String,
    handle: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
    connect_count: Arc<AtomicUsize>,
}

impl SseTestServer {
    /// Start a new SSE test server with a custom per-connection handler.
    ///
    /// The handler receives:
    /// * `&HttpRequest` — the parsed HTTP request (method, path, headers, body)
    /// * `&mut SseStreamWriter` — writer for SSE events
    /// * `usize` — the connection count (0-indexed, so first connection is 0)
    ///
    /// The handler returns an `SseConnectionResult` that determines how the
    /// connection is closed.
    ///
    /// # Panics
    ///
    /// Panics if binding to a local address fails.
    #[must_use]
    pub fn with_handler<F>(handler: F) -> Self
    where
        F: Fn(&HttpRequest, &mut SseStreamWriter, usize) -> SseConnectionResult + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0")
            .expect("Failed to bind SSE test server to localhost");
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let running = Arc::new(AtomicBool::new(true));
        let connect_count = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(std::sync::Mutex::new(
            Box::new(handler) as Box<dyn Fn(&HttpRequest, &mut SseStreamWriter, usize) -> SseConnectionResult + Send>,
        ));

        let running_clone = Arc::clone(&running);
        let handler_clone = Arc::clone(&handler);
        let count_clone = Arc::clone(&connect_count);

        let handle = thread::spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("Failed to set non-blocking");

            while running_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let conn_num = count_clone.fetch_add(1, Ordering::SeqCst);
                        let handler = Arc::clone(&handler_clone);
                        thread::spawn(move || {
                            if let Err(e) = Self::handle_connection(stream, &handler, conn_num) {
                                tracing::info!("SseTestServer connection error: {e}");
                            }
                        });
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => {
                        tracing::info!("SseTestServer accept error: {e}");
                        break;
                    }
                }
            }
        });

        Self {
            addr,
            handle: Some(handle),
            running,
            connect_count,
        }
    }

    /// Convenience: server that sends `count` events then cleanly closes.
    ///
    /// Each event is `{{"seq": N}}` where N is 0-based within the connection.
    #[must_use]
    pub fn events_then_close(count: usize) -> Self {
        Self::with_handler(move |_req: &HttpRequest, writer: &mut SseStreamWriter, _conn: usize| {
            for i in 0..count {
                if writer.event(&format!("{{\"seq\": {i}}}")).is_err() {
                    break;
                }
            }
            SseConnectionResult::clean_close()
        })
    }

    /// Convenience: server that sends one event then abruptly drops.
    ///
    /// Useful for testing reconnection — the abrupt drop triggers a
    /// connection error, not a clean EOF.
    #[must_use]
    pub fn event_then_drop(event_data: &'static str) -> Self {
        Self::with_handler(
            move |_req: &HttpRequest, writer: &mut SseStreamWriter, _conn: usize| {
                let _ = writer.event(event_data);
                SseConnectionResult::abrupt_drop()
            },
        )
    }

    /// Convenience: server that sends one event then writes partial data and drops.
    ///
    /// Useful for testing SSE parse error handling.
    #[must_use]
    pub fn event_then_corrupt(event_data: &'static str) -> Self {
        Self::with_handler(
            move |_req: &HttpRequest, writer: &mut SseStreamWriter, _conn: usize| {
                let _ = writer.event(event_data);
                SseConnectionResult::corrupt_data()
            },
        )
    }

    /// Get the number of connections received so far.
    pub fn connect_count(&self) -> usize {
        self.connect_count.load(Ordering::SeqCst)
    }

    /// Get full URL for a path on this test server.
    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.addr, path)
    }

    /// Get base URL of this test server.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.addr
    }

    fn handle_connection(
        stream: TcpStream,
        handler: &SseResponseHandler,
        conn_num: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Parse the HTTP request using the same infrastructure as TestHttpServer.
        // Clone the stream for reading and set to non-blocking to avoid hangs
        // when reading body streams on blocking TCP.
        let read_stream = stream.try_clone()?;
        read_stream.set_nonblocking(true)?;
        let conn = RawStream::from_tcp(read_stream)?;
        let request_streams = http_streams::send::http_streams(conn);

        let request_reader = request_streams.next_request();

        let parts: Result<Vec<IncomingRequestParts>, HttpReaderError> = request_reader
            .into_iter()
            .filter(|item| !matches!(item, Ok(IncomingRequestParts::SKIP)))
            .collect();

        if let Err(part_err) = parts {
            tracing::error!(
                "SseTestServer: Failed to parse request: {:?}",
                part_err
            );
            return Ok(());
        }

        let mut request_parts = parts.unwrap();
        if request_parts.len() != 3 {
            tracing::error!(
                "SseTestServer: Expected 3 request parts, got {}",
                request_parts.len()
            );
            return Ok(());
        }

        let body_part = request_parts.pop().unwrap();
        let headers_part = request_parts.pop().unwrap();
        let intros_part = request_parts.pop().unwrap();

        let IncomingRequestParts::Intro(method, url, _proto) = intros_part else {
            tracing::error!("SseTestServer: Expected Intro request part");
            return Ok(());
        };

        let IncomingRequestParts::Headers(headers) = headers_part else {
            tracing::error!("SseTestServer: Expected Headers request part");
            return Ok(());
        };

        let body_part = match body_part {
            IncomingRequestParts::NoBody => SendSafeBody::None,
            IncomingRequestParts::SizedBody(body) | IncomingRequestParts::StreamedBody(body) => {
                body
            }
            _ => {
                tracing::error!("SseTestServer: Expected body part");
                return Ok(());
            }
        };

        let body = collect_bytes_from_send_safe(body_part);

        let request = HttpRequest {
            path: url,
            method,
            proto: Proto::HTTP11,
            headers,
            body: SendSafeBody::Bytes(body),
        };

        // Call the user's handler
        let mut writer = SseStreamWriter {
            stream,
            headers_written: false,
        };

        let handler_guard = handler.lock().unwrap();
        let result = (handler_guard)(&request, &mut writer, conn_num);
        drop(handler_guard);

        // Apply close behavior
        match result.close {
            SseCloseBehavior::CleanClose => {
                // Flush and cleanly close (TCP FIN)
                let _ = writer.stream.flush();
                let _ = writer.stream.shutdown(std::net::Shutdown::Write);
            }
            SseCloseBehavior::AbruptDrop => {
                // Set SO_LINGER=0 to send RST on close.
                // This causes the client's next read to return ECONNRESET
                // instead of EOF, triggering reconnection.
                //
                // NOTE: SO_LINGER=0 discards unsent data in the kernel's
                // send buffer. On localhost with a synchronous consumer
                // (foundation_core integration tests) the data is read
                // before the RST arrives. Through the valtron executor
                // the timing differs — the RST may discard buffered data.
                let _ = set_so_linger_zero(&writer.stream);
            }
            SseCloseBehavior::CorruptData => {
                // Write partial SSE data (missing trailing \n\n) without flush
                let _ = write!(writer.stream, "data: partial_event");
                // Don't flush — partial data may or may not reach client
            }
        }

        Ok(())
    }
}

impl Drop for SseTestServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};

    #[test]
    fn test_server_start() {
        let server = SseTestServer::events_then_close(1);
        assert!(server.base_url().starts_with("http://127.0.0.1:"));
        assert_eq!(server.connect_count(), 0);
    }

    #[test]
    fn test_server_drop() {
        {
            let _server = SseTestServer::events_then_close(1);
        }
        // Server should have stopped cleanly
    }

    #[test]
    fn test_custom_handler() {
        let server = SseTestServer::with_handler(
            |_req: &HttpRequest, _writer: &mut SseStreamWriter, _conn: usize| {
                SseConnectionResult::clean_close()
            },
        );
        assert!(server.base_url().starts_with("http://"));
    }

    /// End-to-end: connect via raw TCP and verify SSE events are received.
    #[test]
    fn test_sse_events_received() {
        let server = SseTestServer::events_then_close(2);
        let port = server
            .base_url()
            .strip_prefix("http://127.0.0.1:")
            .unwrap()
            .parse::<u16>()
            .unwrap();

        // Connect with raw TCP
        let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut writer = stream;

        // Send a minimal HTTP/1.1 GET request
        write!(
            writer,
            "GET /test HTTP/1.1\r\nHost: localhost\r\n\r\n"
        )
        .unwrap();
        writer.flush().unwrap();

        // Read the full response
        let mut response = String::new();
        reader.read_to_string(&mut response).unwrap();

        // Should contain HTTP 200 + SSE events
        assert!(response.contains("200 OK"));
        assert!(response.contains("data: {\"seq\": 0}"));
        assert!(response.contains("data: {\"seq\": 1}"));
    }
}
