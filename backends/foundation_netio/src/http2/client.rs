//! HTTP/2 client multiplexer — open connections, negotiate handshake, send
//! requests and receive responses over multiplexed streams (Feature 30).
//!
//! WHY: The client side of HTTP/2: connect to a peer, send the preface,
//! negotiate SETTINGS, then open streams for outgoing requests and read the
//! responses independently.
//!
//! WHAT: [`H2Client`] wraps [`H2Connection`] and exposes a simple
//! request/response API — `send_request` and `recv_response`.
//!
//! HOW: The client allocates odd-numbered stream IDs, sends HEADERS + DATA,
//! and reads response HEADERS + DATA frames from the connection loop.

use std::io::{self, Read, Write};

use crate::http2::connection::{H2Connection, H2Request};

/// Client-side HTTP/2 connection.
///
/// Wraps [`H2Connection`] and provides a synchronous request/response API.
/// For async use, the I/O should be wrapped in non-blocking mode with the
/// `IncrementalDecoder` + valtron parking pattern.
pub struct H2Client<S: Read + Write> {
    conn: H2Connection<S>,
}

impl<S: Read + Write> H2Client<S> {
    /// Create a new client over the given connected socket.
    pub fn new(socket: S) -> Self {
        Self {
            conn: H2Connection::new(socket, false),
        }
    }

    /// Complete the HTTP/2 handshake (send preface, exchange SETTINGS).
    pub fn connect(&mut self) -> io::Result<()> {
        self.conn.client_handshake()
    }

    /// Send an HTTP/2 request on a new stream.
    ///
    /// Returns the stream ID for correlating with the response.
    pub fn send(&mut self, request: H2Request) -> io::Result<u32> {
        self.conn.send_request(request)
    }

    /// Wait for the next response frame from the server.
    ///
    /// Returns `Some((stream_id, response))` for a response HEADERS frame,
    /// or `None` when the connection closes (GOAWAY received).
    pub fn recv(&mut self) -> io::Result<Option<(u32, H2Request)>> {
        self.conn.recv_response()
    }

    /// Flush any pending writes.
    pub fn flush(&mut self) -> io::Result<()> {
        self.conn.flush()
    }

    /// Access the underlying connection.
    pub fn connection(&self) -> &H2Connection<S> {
        &self.conn
    }
    pub fn connection_mut(&mut self) -> &mut H2Connection<S> {
        &mut self.conn
    }
}
