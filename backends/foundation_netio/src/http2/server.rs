//! HTTP/2 server multiplexer — accept connections, complete handshake, fan
//! out per-stream requests to the shared Router (Feature 30, D12 §5 phase 2).
//!
//! WHY: The server side of an HTTP/2 connection: read the client preface,
//! negotiate SETTINGS, then loop reading frames and dispatching each new
//! stream to the handler independently.
//!
//! WHAT: [`H2Server`] wraps [`H2Connection`] and drives the event loop,
//! creating per-stream handler invocations for incoming HEADERS frames.
//!
//! HOW: Uses the [`StreamHandler`] trait from `connection` to bridge between
//! the connection's frame-level events and the application-level handler.

use std::io::{self, Read, Write};

use crate::http2::connection::{H2Connection, H2Request, H2Response, StreamHandler, StreamId};
use crate::http2::frame::ErrorCode;

/// A handler for individual HTTP/2 requests — the server-side equivalent of
/// `foundation_http`'s `Serve` trait, but adapted for direct frame-level use.
pub trait H2Serve: Send + Sync {
    fn serve(&self, request: H2Request) -> H2Response;
}

/// Server-side HTTP/2 connection multiplexer.
///
/// Wraps [`H2Connection`] and drives the frame event loop, dispatching
/// each incoming request to the provided [`H2Serve`] handler.
pub struct H2Server<S: Read + Write> {
    conn: H2Connection<S>,
    handler: Box<dyn H2Serve>,
}

impl<S: Read + Write> H2Server<S> {
    /// Create a new server over the given socket.
    pub fn new(socket: S, handler: Box<dyn H2Serve>) -> Self {
        Self {
            conn: H2Connection::new(socket, true), // is_server = true
            handler,
        }
    }

    /// Run the server: complete the handshake, then process frames until the
    /// connection closes.
    pub fn serve(&mut self) -> io::Result<()> {
        self.conn.server_handshake()?;

        let mut bridge = ServeBridge { handler: &*self.handler };
        loop {
            match self.conn.process_next(&mut bridge) {
                Ok(true) => {} // continue
                Ok(false) => break,
                Err(e) => {
                    // On protocol error, send GOAWAY and close
                    self.conn.send_goaway(0, ErrorCode::ProtocolError);
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Access the underlying connection (for tests/inspection).
    pub fn connection(&self) -> &H2Connection<S> {
        &self.conn
    }

    pub fn connection_mut(&mut self) -> &mut H2Connection<S> {
        &mut self.conn
    }
}

/// Bridge: adapts [`H2Serve`] → [`StreamHandler`] for the connection.
struct ServeBridge<'a> {
    handler: &'a dyn H2Serve,
}

impl StreamHandler for ServeBridge<'_> {
    fn handle_request(&mut self, _stream_id: StreamId, request: H2Request) -> H2Response {
        self.handler.serve(request)
    }
}
