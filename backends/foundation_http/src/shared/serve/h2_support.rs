//! H2Serve trait — the HTTP/2 equivalent of Serve (F47).
//!
//! WHY: Serve::serve() writes Http11 text. For h2, handlers need to write
//! binary frames to an H2Conn on a specific stream.
//!
//! WHAT: A synchronous trait — handlers write response frames into conn
//! and return. The ConnectionHandler owns the conn and handles framing.
//!
//! HOW: The impl (ConnectRpcServeH2 in foundation_connectrpc) calls
//! block_on(handler.dispatch(bag, req)), encodes the SimpleOutgoingResponse
//! as h2 frames via conn, and returns.

use std::sync::Arc;
use foundation_netio::http2::conn::H2Conn;
use foundation_netio::simple_http::shared::SimpleIncomingRequest;
use crate::shared::context::ContextBag;

/// Handler for one h2 stream — the h2 equivalent of Serve.
pub trait H2Serve: Send + Sync + 'static {
    fn serve_h2(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut H2Conn,
        stream_id: u32,
    );
}
