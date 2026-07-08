//! H2Serve trait — the HTTP/2 equivalent of Serve (F47).

use std::sync::Arc;
use foundation_netio::http2::conn::H2Conn;
use foundation_netio::simple_http::shared::SimpleIncomingRequest;
use crate::shared::context::ContextBag;

pub trait H2Serve: Send + Sync + 'static {
    fn serve_h2(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut H2Conn,
        stream_id: u32,
    );
}
