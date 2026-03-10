//! WebSocket connection and client APIs.
//!
//! WHY: Users need both a low-level TaskIterator API and a high-level blocking API
//! for WebSocket communication.
//!
//! WHAT: Provides `WebSocketConnection` (blocking send/recv API) and `WebSocketClient`
//! (consumer wrapper around TaskIterator using executor boundary with send capability).
//!
//! HOW: WebSocketConnection wraps the shared stream directly for blocking operations.
//! WebSocketClient uses `execute_stream()` to integrate with valtron executor and
//! provides `MessageDelivery` for sending messages via ConcurrentQueue.

use crate::io::ioutils::SharedByteBufferStream;
use crate::netcap::RawStream;
use crate::valtron::{execute_stream, DrivenStreamIterator, Stream};
use crate::wire::simple_http::client::DnsResolver;
use crate::wire::simple_http::client::HttpConnectionPool;
use crate::wire::simple_http::SimpleHeader;
use concurrent_queue::ConcurrentQueue;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use super::error::WebSocketError;
use super::frame::{Opcode, WebSocketFrame};
use super::message::WebSocketMessage;
use super::task::WebSocketTask;

/// WHY: Users need a simple blocking API for WebSocket communication.
///
/// WHAT: High-level WebSocket connection with send/recv/close methods.
///
/// HOW: Wraps the shared stream and manages connection state.
pub struct WebSocketConnection {
    stream: SharedByteBufferStream<RawStream>,
    state: ConnectionState,
}

#[allow(dead_code)] // Closed state used in Phase 2
enum ConnectionState {
    Open,
    Closing {
        close_sent: bool,
        close_received: bool,
    },
    Closed,
}

impl WebSocketConnection {
    /// Create a new WebSocketConnection from an established stream.
    ///
    /// WHY: After successful handshake, user needs a connection object.
    /// WHAT: Wraps the stream for frame-based communication.
    #[must_use]
    pub fn new(stream: SharedByteBufferStream<RawStream>) -> Self {
        Self {
            stream,
            state: ConnectionState::Open,
        }
    }

    /// Send a WebSocket message.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the message cannot be sent.
    pub fn send(&mut self, message: WebSocketMessage) -> Result<(), WebSocketError> {
        if matches!(self.state, ConnectionState::Closed) {
            return Err(WebSocketError::ConnectionClosed);
        }

        let frame = match message {
            WebSocketMessage::ConnectionEstablished => {
                return Ok(()); // No frame to send
            }
            WebSocketMessage::Text(text) => WebSocketFrame {
                fin: true,
                opcode: Opcode::Text,
                mask: None, // Client receives unmasked frames from server
                payload: text.into_bytes(),
            },
            WebSocketMessage::Binary(data) => WebSocketFrame {
                fin: true,
                opcode: Opcode::Binary,
                mask: None,
                payload: data,
            },
            WebSocketMessage::Ping(data) => WebSocketFrame {
                fin: true,
                opcode: Opcode::Ping,
                mask: None,
                payload: data,
            },
            WebSocketMessage::Pong(data) => WebSocketFrame {
                fin: true,
                opcode: Opcode::Pong,
                mask: None,
                payload: data,
            },
            WebSocketMessage::Close(code, reason) => {
                let mut payload = code.to_be_bytes().to_vec();
                payload.extend_from_slice(reason.as_bytes());
                WebSocketFrame {
                    fin: true,
                    opcode: Opcode::Close,
                    mask: None,
                    payload,
                }
            }
        };

        self.send_frame(frame)
    }

    /// Receive a WebSocket message.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the message cannot be received.
    pub fn recv(&mut self) -> Result<WebSocketMessage, WebSocketError> {
        if matches!(self.state, ConnectionState::Closed) {
            return Err(WebSocketError::ConnectionClosed);
        }

        let frame = WebSocketFrame::decode(&mut self.stream)?;

        // Validate frame
        frame.validate()?;

        // Client expects unmasked frames from server
        if frame.mask.is_some() {
            return Err(WebSocketError::ProtocolError(
                "Received masked frame from server (protocol violation)".to_string(),
            ));
        }

        // Handle control frames immediately
        if frame.opcode.is_control() {
            return self.handle_control_frame(frame);
        }

        // Data frame - for now, only support non-fragmented messages
        if !frame.fin {
            return Err(WebSocketError::ProtocolError(
                "Fragmented messages not yet supported".to_string(),
            ));
        }

        if frame.opcode == Opcode::Continuation {
            return Err(WebSocketError::ProtocolError(
                "Unexpected continuation frame".to_string(),
            ));
        }

        // Convert frame to message
        return match frame.opcode {
            Opcode::Text => {
                let text = String::from_utf8(frame.payload).map_err(WebSocketError::InvalidUtf8)?;
                Ok(WebSocketMessage::Text(text))
            }
            Opcode::Binary => Ok(WebSocketMessage::Binary(frame.payload)),
            _ => Err(WebSocketError::ProtocolError(
                "Unexpected data frame opcode".to_string(),
            )),
        };
    }

    fn handle_control_frame(
        &mut self,
        frame: WebSocketFrame,
    ) -> Result<WebSocketMessage, WebSocketError> {
        match frame.opcode {
            Opcode::Ping => {
                // Auto-respond with Pong (same payload)
                let pong_frame = WebSocketFrame {
                    fin: true,
                    opcode: Opcode::Pong,
                    mask: None,
                    payload: frame.payload.clone(),
                };
                self.send_frame(pong_frame)?;
                Ok(WebSocketMessage::Ping(frame.payload))
            }
            Opcode::Pong => Ok(WebSocketMessage::Pong(frame.payload)),
            Opcode::Close => {
                let (code, reason) = parse_close_payload(&frame.payload);

                // Send Close response if we haven't already
                if let ConnectionState::Open = self.state {
                    self.state = ConnectionState::Closing {
                        close_sent: false,
                        close_received: true,
                    };
                    self.close(code, &reason)?;
                }

                Ok(WebSocketMessage::Close(code, reason))
            }
            _ => Err(WebSocketError::ProtocolError(
                "Unknown control frame".to_string(),
            )),
        }
    }

    /// Close the WebSocket connection gracefully.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the close frame cannot be sent.
    pub fn close(&mut self, code: u16, reason: &str) -> Result<(), WebSocketError> {
        match self.state {
            ConnectionState::Closed => return Ok(()),
            ConnectionState::Closing {
                close_sent: true, ..
            } => return Ok(()),
            _ => {}
        }

        let mut payload = code.to_be_bytes().to_vec();
        payload.extend_from_slice(reason.as_bytes());

        let frame = WebSocketFrame {
            fin: true,
            opcode: Opcode::Close,
            mask: None,
            payload,
        };
        self.send_frame(frame)?;

        self.state = match self.state {
            ConnectionState::Open => ConnectionState::Closing {
                close_sent: true,
                close_received: false,
            },
            ConnectionState::Closing { close_received, .. } => ConnectionState::Closing {
                close_sent: true,
                close_received,
            },
            ConnectionState::Closed => ConnectionState::Closed,
        };

        Ok(())
    }

    fn send_frame(&mut self, frame: WebSocketFrame) -> Result<(), WebSocketError> {
        let encoded = frame.encode();
        self.stream.write_all(&encoded)?;
        self.stream.flush()?;
        Ok(())
    }

    /// Get an iterator over incoming messages.
    pub fn messages(&mut self) -> ConnectionMessageIterator<'_> {
        ConnectionMessageIterator { conn: self }
    }

    /// Check if the connection is still open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        matches!(self.state, ConnectionState::Open)
    }
}

/// Iterator over messages from a WebSocketConnection.
pub struct ConnectionMessageIterator<'a> {
    conn: &'a mut WebSocketConnection,
}

impl<'a> Iterator for ConnectionMessageIterator<'a> {
    type Item = Result<WebSocketMessage, WebSocketError>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.conn.is_open() {
            return None;
        }
        Some(self.conn.recv())
    }
}

/// Parse close frame payload into status code and reason.
fn parse_close_payload(payload: &[u8]) -> (u16, String) {
    if payload.is_empty() {
        return (1005, String::new()); // No status code present (1005 = No Status Received)
    }
    if payload.len() == 1 {
        return (1002, String::from("Invalid close payload")); // Protocol error
    }

    let code = u16::from_be_bytes([payload[0], payload[1]]);
    let reason = if payload.len() > 2 {
        String::from_utf8_lossy(&payload[2..]).to_string()
    } else {
        String::new()
    };

    (code, reason)
}

// ============== WebSocketClient (Executor-based) ==============

/// WHY: Users need a send-capable WebSocket client that integrates with valtron executor.
///
/// WHAT: `MessageDelivery` provides thread-safe message sending via `ConcurrentQueue`.
///
/// HOW: Wraps `Arc<ConcurrentQueue<WebSocketMessage>>` - cloned for WebSocketTask to read.
#[derive(Clone)]
pub struct MessageDelivery {
    queue: Arc<ConcurrentQueue<WebSocketMessage>>,
}

impl MessageDelivery {
    /// Create a new MessageDelivery with an unbounded queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: Arc::new(ConcurrentQueue::unbounded()),
        }
    }

    /// Send a WebSocket message.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError::ConnectionClosed`] if the queue is disconnected.
    pub fn send(&self, message: WebSocketMessage) -> Result<(), WebSocketError> {
        self.queue
            .push(message)
            .map_err(|_| WebSocketError::ConnectionClosed)?;
        Ok(())
    }

    /// Send a Ping message.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the queue is disconnected.
    pub fn ping(&self, data: Vec<u8>) -> Result<(), WebSocketError> {
        self.send(WebSocketMessage::Ping(data))
    }

    /// Send a Pong message (response to Ping).
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the queue is disconnected.
    pub fn pong(&self, data: Vec<u8>) -> Result<(), WebSocketError> {
        self.send(WebSocketMessage::Pong(data))
    }

    /// Send a Close message.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the queue is disconnected.
    pub fn close(&self, code: u16, reason: &str) -> Result<(), WebSocketError> {
        self.send(WebSocketMessage::Close(code, reason.to_string()))
    }

    /// Get the underlying queue for direct access.
    #[must_use]
    pub fn queue(&self) -> &Arc<ConcurrentQueue<WebSocketMessage>> {
        &self.queue
    }
}

impl Default for MessageDelivery {
    fn default() -> Self {
        Self::new()
    }
}

/// WebSocket event for the client API.
///
/// WHY: Users need to know when the stream is still working vs. when an actual
/// event is available.
///
/// WHAT: Enum with `Message` variant containing the actual message and `Skip` variant
/// for pending/delayed states.
#[derive(Debug, Clone)]
pub enum WebSocketEvent {
    /// A WebSocket message is available.
    Message(WebSocketMessage),
    /// Stream is still working, no message yet. User should call next() again.
    Skip,
}

/// A WebSocket client that uses the valtron executor.
///
/// WHY: Users want to consume WebSocket messages without understanding TaskIterator internals.
///
/// WHAT: Wraps the executor's stream and presents a simple iterator interface.
/// Includes `MessageDelivery` for sending messages.
pub struct WebSocketClient<R: DnsResolver + Send + 'static> {
    inner: DrivenStreamIterator<WebSocketTask<R>>,
    delivery: MessageDelivery,
    read_timeout: Duration,
}

impl<R: DnsResolver + Send + 'static> WebSocketClient<R> {
    /// Connect to a WebSocket endpoint.
    ///
    /// Returns both the client and a `MessageDelivery` handle for sending messages.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if:
    /// - URL is invalid
    /// - Executor fails to schedule the task
    pub fn connect(
        resolver: R,
        url: impl Into<String>,
    ) -> Result<(Self, MessageDelivery), WebSocketError> {
        Self::with_options(resolver, url, None, Vec::new(), Duration::from_secs(1))
    }

    /// Connect to a WebSocket endpoint with custom options.
    ///
    /// # Arguments
    ///
    /// * `resolver` - DNS resolver for hostname resolution
    /// * `url` - WebSocket URL (ws:// or wss://)
    /// * `subprotocols` - Optional comma-separated list of subprotocols
    /// * `extra_headers` - Additional HTTP headers for handshake
    /// * `read_timeout` - Timeout for read operations
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if:
    /// - URL is invalid
    /// - Executor fails to schedule the task
    #[tracing::instrument(name = "websocket_connect", skip(resolver, extra_headers), fields(url))]
    pub fn with_options(
        resolver: R,
        url: impl Into<String>,
        subprotocols: Option<String>,
        extra_headers: Vec<(SimpleHeader, String)>,
        read_timeout: Duration,
    ) -> Result<(Self, MessageDelivery), WebSocketError> {
        let url_str = url.into();
        let delivery = MessageDelivery::new();
        let task = WebSocketTask::connect_with_delivery(
            resolver,
            url_str,
            subprotocols,
            extra_headers,
            delivery.queue().clone(),
            read_timeout,
        )?;
        let inner = execute_stream(task, None)
            .map_err(|e| WebSocketError::ProtocolError(format!("Executor error: {e}")))?;
        let client = Self {
            inner,
            delivery: delivery.clone(),
            read_timeout,
        };
        Ok((client, delivery))
    }

    /// Connect using an existing connection pool.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if:
    /// - URL is invalid
    /// - Executor fails to schedule the task
    pub fn with_pool(
        url: impl Into<String>,
        pool: Arc<HttpConnectionPool<R>>,
    ) -> Result<(Self, MessageDelivery), WebSocketError> {
        Self::with_pool_and_options(url, pool, None, Vec::new(), Duration::from_secs(1))
    }

    /// Connect using an existing connection pool with custom options.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if:
    /// - URL is invalid
    /// - Executor fails to schedule the task
    pub fn with_pool_and_options(
        url: impl Into<String>,
        pool: Arc<HttpConnectionPool<R>>,
        subprotocols: Option<String>,
        extra_headers: Vec<(SimpleHeader, String)>,
        read_timeout: Duration,
    ) -> Result<(Self, MessageDelivery), WebSocketError> {
        let url_str = url.into();
        let delivery = MessageDelivery::new();
        let task = WebSocketTask::connect_with_pool_and_delivery(
            url_str,
            pool,
            subprotocols,
            extra_headers,
            delivery.queue().clone(),
            read_timeout,
        )?;
        let inner = execute_stream(task, None)
            .map_err(|e| WebSocketError::ProtocolError(format!("Executor error: {e}")))?;
        let client = Self {
            inner,
            delivery: delivery.clone(),
            read_timeout,
        };
        Ok((client, delivery))
    }

    /// Get the message delivery handle for sending messages.
    ///
    /// This can be cloned and shared across threads.
    #[must_use]
    pub fn delivery(&self) -> MessageDelivery {
        self.delivery.clone()
    }

    /// Get an iterator over incoming messages.
    pub fn messages(&mut self) -> WebSocketMessageIterator<'_, R> {
        WebSocketMessageIterator { client: self }
    }

    /// Send a WebSocket message (convenience wrapper around delivery.send()).
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the message cannot be sent.
    pub fn send(&self, message: WebSocketMessage) -> Result<(), WebSocketError> {
        self.delivery.send(message)
    }

    /// Receive a WebSocket message (blocking).
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the message cannot be received.
    pub fn recv(&mut self) -> Result<WebSocketMessage, WebSocketError> {
        loop {
            match self.inner.next() {
                Some(Stream::Next(result)) => {
                    match result {
                        Ok(WebSocketMessage::ConnectionEstablished) => continue, // Skip connection established
                        other => return other,
                    }
                }
                Some(Stream::Init) | Some(Stream::Ignore) | Some(Stream::Pending(_)) | Some(Stream::Delayed(_)) => {
                    continue; // Keep waiting
                }
                None => return Err(WebSocketError::ConnectionClosed),
            }
        }
    }

    /// Close the WebSocket connection (convenience wrapper around delivery.close()).
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the close message cannot be sent.
    pub fn close(&self, code: u16, reason: &str) -> Result<(), WebSocketError> {
        self.delivery.close(code, reason)
    }

    /// Check if the connection is still open.
    pub fn is_open(&self) -> bool {
        // The client is considered "open" if we can still send messages
        // The actual connection state is managed by the WebSocketTask
        true
    }
}

/// Iterator over messages from a WebSocketClient.
pub struct WebSocketMessageIterator<'a, R: DnsResolver + Send + 'static> {
    client: &'a mut WebSocketClient<R>,
}

impl<'a, R: DnsResolver + Send + 'static> Iterator for WebSocketMessageIterator<'a, R> {
    type Item = Result<WebSocketEvent, WebSocketError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.client.inner.next()? {
            Stream::Next(result) => {
                // Skip ConnectionEstablished message, return actual messages
                match result {
                    Ok(WebSocketMessage::ConnectionEstablished) => Some(Ok(WebSocketEvent::Skip)),
                    other => Some(other.map(WebSocketEvent::Message)),
                }
            }
            Stream::Init | Stream::Ignore | Stream::Pending(_) | Stream::Delayed(_) => {
                Some(Ok(WebSocketEvent::Skip))
            }
        }
    }
}
