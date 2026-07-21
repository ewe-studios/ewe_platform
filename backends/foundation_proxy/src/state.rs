//! Shared proxy runtime state.
//!
//! WHY: The HTTP framework constructs handlers via `ServeFactory::create(&bag)`,
//! so all proxy state the handler needs — the router table, backend health and
//! load-balancer counters, and the pooled upstream client — must live in the
//! `ContextBag`. One `Arc<ProxyState>` carries it all.
//!
//! WHAT: [`ProxyState`] holds the [`Router`] and the shared [`SharedHttpClient`],
//! plus the scheme the front end terminates (used for `X-Forwarded-Proto`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use foundation_iogate::ServerIo;

use crate::config::BackendState;
use crate::forward::SharedHttpClient;
use crate::persistence::ProxyStateStore;
use crate::router::Router;

/// Immutable-after-startup proxy state shared by every request handler.
pub struct ProxyState {
    router: Arc<Router>,
    client: SharedHttpClient,
    scheme: String,
    io_mode: ServerIo,
    /// When true, the accept loop has stopped and in-flight requests are draining.
    /// The handler checks this before routing new requests — if draining, it
    /// responds 503 to signal the client to retry elsewhere (zero-downtime deploy,
    /// Decision 22).
    draining: AtomicBool,
    /// Optional persistence store (Decision 21, F14). When present, admin state
    /// changes (drain/pause/activate) are written through so they survive a
    /// restart. `None` disables persistence.
    store: Option<Arc<ProxyStateStore>>,
}

impl std::fmt::Debug for ProxyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyState")
            .field("router", &self.router)
            .field("scheme", &self.scheme)
            .finish_non_exhaustive()
    }
}

impl ProxyState {
    /// Build proxy state from a router and pooled client.
    ///
    /// `scheme` is what the front end speaks to clients (`http` in stage 1); it
    /// becomes the `X-Forwarded-Proto` value sent upstream.
    #[must_use]
    pub fn new(
        router: Arc<Router>,
        client: SharedHttpClient,
        scheme: impl Into<String>,
        io_mode: ServerIo,
    ) -> Self {
        Self {
            router,
            client,
            scheme: scheme.into(),
            io_mode,
            draining: AtomicBool::new(false),
            store: None,
        }
    }

    /// Attach a persistence store (Decision 21). Admin state changes are then
    /// written through so drain/pause survive a restart.
    #[must_use]
    pub fn with_store(mut self, store: Arc<ProxyStateStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Persist a backend's state change, if a store is attached. Best-effort:
    /// a persistence failure is logged, never propagated to the admin caller.
    pub fn persist_backend_state(&self, service: &str, url: &str, state: BackendState) {
        if let Some(store) = &self.store {
            if let Err(e) = store.save_backend_state(service, url, &state.to_string()) {
                tracing::warn!(%service, %url, "failed to persist backend state: {e}");
            }
        }
    }

    /// Begin connection draining — stop routing new requests.
    pub fn start_drain(&self) {
        self.draining.store(true, Ordering::SeqCst);
    }

    /// Whether the proxy is currently draining.
    #[must_use]
    pub fn is_draining(&self) -> bool {
        self.draining.load(Ordering::SeqCst)
    }

    /// The total in-flight request count across all services/backends.
    #[must_use]
    pub fn inflight_total(&self) -> u32 {
        self.router
            .services()
            .iter()
            .flat_map(|s| s.backends())
            .map(|b| b.inflight())
            .sum()
    }

    #[must_use]
    pub fn router(&self) -> &Arc<Router> {
        &self.router
    }

    #[must_use]
    pub fn client(&self) -> &SharedHttpClient {
        &self.client
    }

    #[must_use]
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    /// The I/O mode for dialing upstream legs (F50). `Completion` dials through
    /// `iogate::connect_completion` so the upstream reads from the io_uring inbox
    /// and writes via `IORING_OP_SEND`.
    #[must_use]
    pub fn io_mode(&self) -> ServerIo {
        self.io_mode
    }
}
