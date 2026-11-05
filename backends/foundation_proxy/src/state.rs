//! Shared proxy runtime state.
//!
//! WHY: The HTTP framework constructs handlers via `ServeFactory::create(&bag)`,
//! so all proxy state the handler needs — the router table, backend health and
//! load-balancer counters, and the pooled upstream client — must live in the
//! `ContextBag`. One `Arc<ProxyState>` carries it all.
//!
//! WHAT: [`ProxyState`] holds the [`Router`] and the shared [`SharedHttpClient`],
//! plus the scheme the front end terminates (used for `X-Forwarded-Proto`).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_http::native::writer::WriterConfig;
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
    /// Limits for the response writer of each relayed connection.
    writer: WriterConfig,
    /// The largest relayed request body in bytes; 0 means no limit.
    max_request_body: usize,
    /// The longest a backend TCP connect may take.
    connect_timeout: Duration,
    /// The longest an H3 client may take no byte of a request head or body.
    client_read_timeout: Duration,
    /// Response body bytes the H3 relay holds in memory (see
    /// [`BufferedBytesGauge`]).
    h3_body_gauge: BufferedBytesGauge,
}

/// A gauge of body bytes a relay holds in memory at once.
///
/// WHY: bodies are streamed, never buffered (rule 4). A relay that streams holds
/// at most one chunk at a time. The gauge makes that testable: a relay calls
/// [`Self::add`] when it reads a chunk from one side and [`Self::sub`] when the
/// other side has taken it; [`Self::peak`] is the most it held at once.
#[derive(Debug, Default)]
pub struct BufferedBytesGauge {
    current: AtomicUsize,
    peak: AtomicUsize,
    polls: AtomicUsize,
}

impl BufferedBytesGauge {
    /// A relay now holds `bytes` more.
    pub fn add(&self, bytes: usize) {
        let now = self.current.fetch_add(bytes, Ordering::SeqCst) + bytes;
        self.peak.fetch_max(now, Ordering::SeqCst);
    }

    /// A relay no longer holds `bytes`.
    pub fn sub(&self, bytes: usize) {
        self.current.fetch_sub(bytes, Ordering::SeqCst);
    }

    /// A relay polled the client's stream for input once.
    pub fn note_poll(&self) {
        self.polls.fetch_add(1, Ordering::SeqCst);
    }

    /// How many times relays polled the client's stream for input. An idle
    /// request that parks on the connection's waker adds almost none; one that
    /// polls on a timer adds one per tick.
    #[must_use]
    pub fn polls(&self) -> usize {
        self.polls.load(Ordering::SeqCst)
    }

    /// The bytes held now.
    #[must_use]
    pub fn current(&self) -> usize {
        self.current.load(Ordering::SeqCst)
    }

    /// The most bytes held at once since the proxy started.
    #[must_use]
    pub fn peak(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }
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
            writer: WriterConfig::default(),
            max_request_body: 1024 * 1024,
            connect_timeout: Duration::from_secs(60),
            client_read_timeout: Duration::from_secs(60),
            h3_body_gauge: BufferedBytesGauge::default(),
        }
    }

    /// Set the largest relayed request body in bytes (0 = no limit). Default
    /// 1 MiB (nginx `client_max_body_size`).
    #[must_use]
    pub fn with_max_request_body(mut self, limit: usize) -> Self {
        self.max_request_body = limit;
        self
    }

    /// The largest relayed request body in bytes; 0 means no limit.
    #[must_use]
    pub fn max_request_body(&self) -> usize {
        self.max_request_body
    }

    /// Set the longest an H3 client may take no byte of a request head or body.
    /// Default 60 seconds (nginx `client_body_timeout`).
    #[must_use]
    pub fn with_client_read_timeout(mut self, timeout: Duration) -> Self {
        self.client_read_timeout = timeout;
        self
    }

    /// The longest an H3 client may take no byte of a request head or body.
    #[must_use]
    pub fn client_read_timeout(&self) -> Duration {
        self.client_read_timeout
    }

    /// The gauge of response body bytes the H3 relay holds in memory.
    #[must_use]
    pub fn h3_body_gauge(&self) -> &BufferedBytesGauge {
        &self.h3_body_gauge
    }

    /// Set the longest a backend TCP connect may take. Default 60 seconds
    /// (nginx `proxy_connect_timeout`).
    #[must_use]
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// The longest a backend TCP connect may take.
    #[must_use]
    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    /// Set the limits of the response writer each relay uses (the send
    /// timeout).
    #[must_use]
    pub fn with_writer_config(mut self, writer: WriterConfig) -> Self {
        self.writer = writer;
        self
    }

    /// The limits of the response writer each relay uses.
    #[must_use]
    pub fn writer_config(&self) -> WriterConfig {
        self.writer
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
    pub async fn persist_backend_state(&self, service: &str, url: &str, state: BackendState) {
        if let Some(store) = &self.store {
            if let Err(e) = store
                .save_backend_state(service, url, &state.to_string())
                .await
            {
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
    /// `iogate::connect_completion` so the upstream reads from the `io_uring` inbox
    /// and writes via `IORING_OP_SEND`.
    #[must_use]
    pub fn io_mode(&self) -> ServerIo {
        self.io_mode
    }
}
