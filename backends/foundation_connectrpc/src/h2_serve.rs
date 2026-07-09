//! `ConnectRpcServeH2` — bridges ConnectRPC dispatch to h2 stream frames (F47).
//!
//! WHY: `ConnectRpcServe` implements `Serve` for HTTP/1.1. This does the same for
//! HTTP/2 via `H2Serve`. It mirrors that layering exactly — the trait lives in
//! `foundation_http`, the impl here — so there is no dependency inversion.
//!
//! WHAT: [`ConnectRpcServeH2`] wrapping a [`ConnectRpcHandler`].
//!
//! HOW: `serve_h2()` returns the future `ConnectRpcHandler::dispatch_h2()`
//! builds. The connection handler spawns it on the valtron pool, so nothing here
//! blocks and every stream on a connection makes progress independently.

use std::io;
use std::sync::Arc;

use foundation_core::valtron::{PipeReceiver, PipeSender};
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::h2::{BoxFuture, H2Serve};
use foundation_netio::http2::types::{H2Frame, H2IncomingFrame, SimpleIncomingRequestHeader};

use crate::router::ConnectRpcHandler;

/// `H2Serve` impl that dispatches through a ConnectRPC router.
pub struct ConnectRpcServeH2 {
    handler: Arc<ConnectRpcHandler>,
}

impl ConnectRpcServeH2 {
    #[must_use]
    pub fn new(handler: ConnectRpcHandler) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }
}

impl H2Serve for ConnectRpcServeH2 {
    fn serve_h2(
        &self,
        bag: Arc<ContextBag>,
        header: SimpleIncomingRequestHeader,
        body: PipeReceiver<H2IncomingFrame>,
        tx: PipeSender<H2Frame>,
    ) -> BoxFuture<'static, io::Result<()>> {
        self.handler.dispatch_h2(bag, header, body, tx)
    }
}
