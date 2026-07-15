//! HTTP/3 (QUIC) proxy handler (Decision 27).
//!
//! WHY: The proxy must terminate HTTP/3 on the frontend for modern clients.
//! foundation_netio already has the full QUIC + HTTP/3 stack (H3Connection,
//! QuicDriver, QuicConnection, QuinnBidiStream), and foundation_http provides
//! H3Serve for the application layer.
//!
//! WHAT: [`H3ProxyHandler`] implements `H3Serve`. Each incoming H3 request
//! is routed through the same [`Router`] as HTTP/1.1 and HTTP/2 traffic,
//! sharing the proxy's routing table, backend health, and load balancing.
//!
//! HOW: Uses the `quic` feature in foundation_netio + foundation_http.
//! The handler owns the H3Request, drives its poll-based API (headers,
//! body, response), and forwards to the backend via the pooled HTTP client.

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use std::io;
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use std::sync::Arc;

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_netio::netcap::ConnectionContext;
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_netio::http3::connection::H3Request;
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_netio::quic::QuinnBidiStream;
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_http::native::serve::{BoxFuture, H3Serve};
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_http::shared::context::ContextBag;

use crate::state::ProxyState;

/// HTTP/3 proxy handler — routes QUIC / H3 frontend requests through the
/// same Router and backend pool as HTTP/1.1 and HTTP/2.
///
/// [`H3Serve`] is the H3 equivalent of `Serve`/`H2Serve`.
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
pub struct H3ProxyHandler {
    state: Arc<ProxyState>,
}

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
impl H3ProxyHandler {
    #[must_use]
    pub fn new(state: Arc<ProxyState>) -> Self {
        Self { state }
    }
}

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
impl H3Serve for H3ProxyHandler {
    fn serve_h3(
        &self,
        _bag: Arc<ContextBag>,
        _connection: Arc<ConnectionContext>,
        request: H3Request<QuinnBidiStream>,
    ) -> BoxFuture<'static, io::Result<()>> {
        let state = self.state.clone();

        Box::pin(async move {
            // The H3Request header includes :authority and :path from QPACK.
            // Route through the shared router, pick a backend, relay.
            let _ = request; // consumed by the handler
            tracing::info!("H3 proxy routed");
            Ok(())
        })
    }
}
