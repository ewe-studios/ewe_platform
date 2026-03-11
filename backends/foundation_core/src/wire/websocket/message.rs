//! WebSocket message types.
//!
//! WHY: Applications work with complete messages, not raw frames.
//! WHAT: Defines [`WebSocketMessage`] enum representing high-level WebSocket message types.

/// WHY: Callers need a clean API for sending/receiving WebSocket messages without dealing
/// with frame-level details like fragmentation or opcodes.
///
/// WHAT: Represents a complete WebSocket message (text, binary, ping, pong, or close).
///
/// HOW: Each variant carries the appropriate payload type for its message kind.
///
/// # Panics
/// Never panics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSocketMessage {
    /// Connection successfully established.
    ConnectionEstablished,

    /// UTF-8 text message.
    Text(String),

    /// Binary message.
    Binary(Vec<u8>),

    /// Ping control message with optional payload.
    Ping(Vec<u8>),

    /// Pong control message with optional payload.
    Pong(Vec<u8>),

    /// Close control message with status code and reason.
    Close(u16, String),
}

impl WebSocketMessage {
    /// WHY: Tracing and logging need human-readable message type names.
    ///
    /// WHAT: Returns the variant name as a string.
    ///
    /// HOW: Matches on self and returns static string for each variant.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn variant_name(&self) -> &'static str {
        match self {
            WebSocketMessage::ConnectionEstablished => "connection_established",
            WebSocketMessage::Text(_) => "text",
            WebSocketMessage::Binary(_) => "binary",
            WebSocketMessage::Ping(_) => "ping",
            WebSocketMessage::Pong(_) => "pong",
            WebSocketMessage::Close(_, _) => "close",
        }
    }
}
