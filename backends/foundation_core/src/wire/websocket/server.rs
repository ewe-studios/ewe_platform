//! WebSocket server-side upgrade handling.
//!
//! WHY: Servers need to accept WebSocket upgrade requests and respond with proper 101 Switching Protocols.
//! WHAT: Provides `WebSocketUpgrade` for detecting and accepting WebSocket upgrades, and
//! `WebSocketServerConnection` for sending/receiving frames after upgrade.
//!
//! HOW: Checks Upgrade/Connection headers, validates Sec-WebSocket-Key, computes accept key,
//! builds 101 response. Server connection does NOT mask outgoing frames (per RFC 6455).

use crate::io::ioutils::SharedByteBufferStream;
use crate::netcap::RawStream;
use crate::wire::simple_http::{Http11, RenderHttp, SimpleHeader, SimpleOutgoingResponse, SimpleIncomingRequest, Status};

use super::batch_writer::BatchFrameWriter;
use super::error::WebSocketError;
use super::frame::{Opcode, WebSocketFrame};
use super::handshake::compute_accept_key;
use super::message::WebSocketMessage;

/// WHY: Servers need to detect incoming WebSocket upgrade requests.
///
/// WHAT: `WebSocketUpgrade` provides methods to detect and accept WebSocket upgrades.
///
/// HOW: Validates required headers per RFC 6455 Section 4.2.
pub struct WebSocketUpgrade;

impl WebSocketUpgrade {
    /// Check if an incoming HTTP request is a WebSocket upgrade request.
    ///
    /// # Requirements (RFC 6455 Section 4.2.1)
    ///
    /// - Method MUST be GET
    /// - Upgrade header MUST contain "websocket" (case-insensitive)
    /// - Connection header MUST contain "Upgrade" (case-insensitive)
    /// - Sec-WebSocket-Key MUST be present
    /// - Sec-WebSocket-Version MUST be "13"
    #[must_use]
    pub fn is_upgrade_request(request: &SimpleIncomingRequest) -> bool {
        // Method must be GET
        let method_str = request.method.to_string();
        if method_str != "GET" {
            return false;
        }

        // Check Upgrade header contains "websocket" (case-insensitive)
        let upgrade_valid = request
            .headers
            .get(&SimpleHeader::UPGRADE)
            .and_then(|values| values.first())
            .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));

        if !upgrade_valid {
            return false;
        }

        // Check Connection header contains "Upgrade" (case-insensitive)
        let connection_valid = request
            .headers
            .get(&SimpleHeader::CONNECTION)
            .is_some_and(|values| values.iter().any(|v| v.eq_ignore_ascii_case("upgrade")));

        if !connection_valid {
            return false;
        }

        // Check Sec-WebSocket-Key is present
        if !request.headers.contains_key(&SimpleHeader::SEC_WEBSOCKET_KEY) {
            return false;
        }

        // Check Sec-WebSocket-Version is "13"
        let version_valid = request
            .headers
            .get(&SimpleHeader::SEC_WEBSOCKET_VERSION)
            .and_then(|values| values.first())
            .is_some_and(|v| v == "13");

        if !version_valid {
            return false;
        }

        true
    }

    /// Extract the Sec-WebSocket-Key from a validated upgrade request.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError::MissingKey`] if the key is not present.
    pub fn extract_key(request: &SimpleIncomingRequest) -> Result<String, WebSocketError> {
        request
            .headers
            .get(&SimpleHeader::SEC_WEBSOCKET_KEY)
            .and_then(|values| values.first())
            .cloned()
            .ok_or(WebSocketError::MissingKey)
    }

    /// Extract optional subprotocols from the request.
    ///
    /// The Sec-WebSocket-Protocol header contains a comma-separated list
    /// of subprotocols the client wants to use.
    #[must_use]
    pub fn extract_subprotocols(request: &SimpleIncomingRequest) -> Option<String> {
        request
            .headers
            .get(&SimpleHeader::SEC_WEBSOCKET_PROTOCOL)
            .and_then(|values| values.first())
            .cloned()
    }

    /// Accept a WebSocket upgrade request and build the 101 Switching Protocols response.
    ///
    /// # Arguments
    ///
    /// * `request` - The validated upgrade request
    /// * `selected_subprotocol` - Optional subprotocol to include in response (must be from client's list)
    ///
    /// # Returns
    ///
    /// A tuple of (response bytes, computed accept key) for the server to send.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError::MissingKey`] if the request key is missing.
    pub fn accept(
        request: &SimpleIncomingRequest,
        selected_subprotocol: Option<&str>,
    ) -> Result<(Vec<Vec<u8>>, String), WebSocketError> {
        let client_key = Self::extract_key(request)?;
        let accept_key = compute_accept_key(&client_key);

        // Build 101 Switching Protocols response using SimpleOutgoingResponse builder
        let mut builder = SimpleOutgoingResponse::builder()
            .with_status(Status::SwitchingProtocols)
            .add_header(SimpleHeader::UPGRADE, "websocket")
            .add_header(SimpleHeader::CONNECTION, "Upgrade")
            .add_header(SimpleHeader::SEC_WEBSOCKET_ACCEPT, &accept_key);

        // Optional subprotocol (server selects one from client's list)
        if let Some(protocol) = selected_subprotocol {
            builder = builder.add_header(SimpleHeader::SEC_WEBSOCKET_PROTOCOL, protocol);
        }

        let response = builder
            .build()
            .map_err(|e| WebSocketError::ProtocolError(format!("Failed to build response: {}", e)))?;

        // Render response to bytes
        let response_bytes_vec = Http11::response(response)
            .http_render_string()
            .map_err(|e| WebSocketError::ProtocolError(format!("Failed to render response: {}", e)))?;

        let response_bytes: Vec<Vec<u8>> = response_bytes_vec
            .into_bytes()
            .into_iter()
            .map(|b| vec![b])
            .collect();

        Ok((response_bytes, accept_key))
    }
}

/// WHY: After accepting a WebSocket upgrade, the server needs a connection object
/// for sending and receiving frames.
///
/// WHAT: `WebSocketServerConnection` wraps the shared stream for frame-based communication.
/// Server connections do NOT mask outgoing frames (per RFC 6455 Section 5.3).
///
/// HOW: Wraps `SharedByteBufferStream<RawStream>` and manages connection state.
/// Outgoing frames are encoded without masking.
/// Uses BatchFrameWriter for efficient frame writing.
pub struct WebSocketServerConnection {
    stream: SharedByteBufferStream<RawStream>,
    writer: BatchFrameWriter<SharedByteBufferStream<RawStream>>,
    state: ServerConnectionState,
}

#[allow(dead_code)] // Closing/Closed states used in more advanced scenarios
enum ServerConnectionState {
    Open,
    Closing {
        close_sent: bool,
        close_received: bool,
    },
    Closed,
}

impl WebSocketServerConnection {
    /// Create a new server connection from an established stream.
    ///
    /// # Arguments
    ///
    /// * `stream` - The shared byte buffer stream after successful handshake
    #[must_use]
    pub fn new(stream: SharedByteBufferStream<RawStream>) -> Self {
        let writer = BatchFrameWriter::with_defaults(stream.clone());
        Self {
            stream,
            writer,
            state: ServerConnectionState::Open,
        }
    }

    /// Send a WebSocket frame. Server frames are NOT masked.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the frame cannot be sent.
    pub fn send_frame(&mut self, mut frame: WebSocketFrame) -> Result<(), WebSocketError> {
        if matches!(self.state, ServerConnectionState::Closed) {
            return Err(WebSocketError::ConnectionClosed);
        }

        // Server MUST NOT mask outgoing frames (RFC 6455 Section 5.3)
        frame.mask = None;

        // Control frames (Pong, Close) should be sent immediately
        // to avoid buffering delays for time-sensitive responses
        if frame.opcode.is_control() {
            self.writer.write_immediate(frame)
        } else {
            self.writer.queue_frame(frame)
        }
    }

    /// Send a WebSocket message (convenience wrapper around send_frame).
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the message cannot be sent.
    pub fn send(&mut self, message: WebSocketMessage) -> Result<(), WebSocketError> {
        let frame = match message {
            WebSocketMessage::ConnectionEstablished => {
                return Ok(()); // No frame to send
            }
            WebSocketMessage::Text(text) => WebSocketFrame {
                fin: true,
                opcode: Opcode::Text,
                mask: None, // Server never masks
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
        self.send_frame(frame)?;
        // Flush after sending a complete message to ensure timely delivery
        self.writer.flush()
    }

    /// Receive the next WebSocket frame.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the frame cannot be read or is invalid.
    pub fn recv_frame(&mut self) -> Result<WebSocketFrame, WebSocketError> {
        if matches!(self.state, ServerConnectionState::Closed) {
            return Err(WebSocketError::ConnectionClosed);
        }

        // Flush any pending frames before reading
        self.writer.flush()?;

        let frame = WebSocketFrame::decode(&mut self.stream)?;

        // Server MUST reject unmasked frames from client with close code 1002
        if frame.mask.is_none() && !frame.opcode.is_control() {
            // For data frames, client MUST mask
            // Send close frame with protocol error code
            let close_frame = WebSocketFrame {
                fin: true,
                opcode: Opcode::Close,
                mask: None,
                payload: {
                    let mut payload = 1002u16.to_be_bytes().to_vec();
                    payload.extend_from_slice(b"unmasked frame");
                    payload
                },
            };
            let _ = self.send_frame(close_frame);
            return Err(WebSocketError::unmasked_client_frame());
        }

        Ok(frame)
    }

    /// Receive the next WebSocket message (handles frame assembly).
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the message cannot be read or is invalid.
    pub fn recv(&mut self) -> Result<WebSocketMessage, WebSocketError> {
        let frame = self.recv_frame()?;
        frame.to_message()
    }

    /// Close the WebSocket connection gracefully.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the close frame cannot be sent.
    pub fn close(&mut self, code: u16, reason: &str) -> Result<(), WebSocketError> {
        match self.state {
            ServerConnectionState::Closed => return Ok(()),
            ServerConnectionState::Closing {
                close_sent: true, ..
            } => return Ok(()),
            _ => {}
        }

        // Flush any pending frames before sending close
        self.writer.flush()?;

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
            ServerConnectionState::Open => ServerConnectionState::Closing {
                close_sent: true,
                close_received: false,
            },
            ServerConnectionState::Closing { close_received, .. } => {
                ServerConnectionState::Closing {
                    close_sent: true,
                    close_received,
                }
            }
            ServerConnectionState::Closed => ServerConnectionState::Closed,
        };

        Ok(())
    }

    /// Get an iterator over incoming messages.
    pub fn messages(&mut self) -> ServerMessageIterator<'_> {
        ServerMessageIterator { conn: self }
    }

    /// Check if the connection is still open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        matches!(self.state, ServerConnectionState::Open)
    }

    /// Flush any pending frames in the batch writer.
    ///
    /// WHY: Ensures queued frames are immediately transmitted.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the flush fails.
    pub fn flush(&mut self) -> Result<(), WebSocketError> {
        self.writer.flush()
    }

    /// Get writer statistics (if batch writing is enabled).
    #[must_use]
    pub fn writer_stats(&self) -> crate::wire::websocket::BatchWriterStats {
        self.writer.stats()
    }

    /// Get the underlying stream for advanced operations.
    #[must_use]
    pub fn stream(&self) -> SharedByteBufferStream<RawStream> {
        self.stream.clone()
    }
}

/// Iterator over messages from a WebSocketServerConnection.
pub struct ServerMessageIterator<'a> {
    conn: &'a mut WebSocketServerConnection,
}

impl<'a> Iterator for ServerMessageIterator<'a> {
    type Item = Result<WebSocketMessage, WebSocketError>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.conn.is_open() {
            return None;
        }
        Some(self.conn.recv())
    }
}

/// WebSocket error variant for unmasked client frames (server-side validation).
impl WebSocketError {
    /// Server received unmasked frame from client.
    ///
    /// # Errors
    ///
    /// Returns a ProtocolError indicating the masking violation.
    pub fn unmasked_client_frame() -> Self {
        WebSocketError::ProtocolError("Client sent unmasked frame".to_string())
    }
}
