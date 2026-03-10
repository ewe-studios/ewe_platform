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
