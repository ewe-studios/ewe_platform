//! Server connection-owner adapter (Decision 11 §Connection ownership, Decision 08
//! §0) — native only.
//!
//! WHY: The router (Decision 08) decides and executes an RPC, but something has to
//! own the raw connection and move bytes to/from the socket. On foundation_netio's
//! server that owner is the **per-connection task the listener spawns on `accept`**
//! (`Serve`): it has already read the request off the wire and hands us a complete
//! `SimpleIncomingRequest` plus the connection to write the response to. This
//! adapter is the bridge — it makes a [`ConnectRpcHandler`] a mountable `Serve`
//! handler, so the connection owner drives dispatch and renders the response.
//!
//! WHAT: [`ConnectRpcServe`] — wraps a frozen [`ConnectRpcHandler`] and implements
//! foundation_http's `Serve`.
//!
//! HOW: `serve` runs the dispatch future to completion (it is self-contained — the
//! streaming path joins its own tasks cooperatively, Decision 08 — so it needs no
//! executor pool) and renders the `SimpleOutgoingResponse` to the connection with
//! `Http11`. A live streaming transport (feature 44's streaming half) later swaps
//! the buffered render for a response pump; unary is complete as-is.

use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve};
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{
    Http11, RenderHttp, SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleOutgoingResponse,
};

use crate::shared::router::ConnectRpcHandler;

/// A [`ConnectRpcHandler`] mounted as a foundation_http `Serve` handler — the
/// server-side connection-owner bridge (Decision 11 §Connection ownership).
pub struct ConnectRpcServe {
    handler: Arc<ConnectRpcHandler>,
}

impl ConnectRpcServe {
    /// Wrap a frozen handler (from [`Router::into_handler`](crate::Router::into_handler)).
    #[must_use]
    pub fn new(handler: ConnectRpcHandler) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }

    /// Share an already-`Arc`'d handler across mounts.
    #[must_use]
    pub fn from_arc(handler: Arc<ConnectRpcHandler>) -> Self {
        Self { handler }
    }
}

impl Serve for ConnectRpcServe {
    fn serve(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        // The connection owner (the netio server) already read the request body and
        // handed us a complete request; produce the response and write it back.
        let mut response = block_on(self.handler.dispatch(bag, req));
        // Frame the wire: a sized body needs an explicit Content-Length (netio's
        // renderer does not compute it). The connection owner owns wire framing.
        ensure_content_length(&mut response);
        match Http11::response(response).http_render_to_writer(&mut conn) {
            Ok(_) => ConnectionResult::Keep,
            Err(_) => ConnectionResult::Close(None),
        }
    }
}

/// Set `Content-Length` from a sized (`Bytes`/`Text`) body when it is not already
/// present, so the response is a well-formed, self-delimiting HTTP/1.1 message.
fn ensure_content_length(response: &mut SimpleOutgoingResponse) {
    let len = match &response.body {
        Some(SendSafeBody::Bytes(bytes)) => Some(bytes.len()),
        Some(SendSafeBody::Text(text)) => Some(text.len()),
        _ => None,
    };
    if let Some(len) = len {
        if response.headers.get(&SimpleHeader::CONTENT_LENGTH).is_none() {
            response
                .headers
                .insert(SimpleHeader::CONTENT_LENGTH, vec![len.to_string()]);
        }
    }
}

/// Drive a self-contained future to completion by parking the current thread.
///
/// The dispatch future never spawns external tasks (Decision 08 — the streaming
/// path joins its own sub-futures cooperatively), so polling it to completion is
/// all that is required; no executor pool is involved.
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}
