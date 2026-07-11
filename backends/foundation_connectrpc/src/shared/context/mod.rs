//! The per-call context model (Decision 04): [`Ctx`], [`RequestContext`],
//! [`CancelSignal`], and the extensions travel pathway.
//!
//! WHY: Every handler and client method receives a [`Ctx`] **by value**. It is
//! owned + cheaply-`Clone` (Arc-backed internals): a clone is a handful of
//! refcount bumps and `'static`, which is what lets streaming handlers return
//! `impl Stream + Send + 'static`. **Write = rebuild-and-move, read = share** —
//! after construction the shared parts are immutable; a layer that changes
//! something calls a `with_*` method (copy-on-write) and passes the new `Ctx`
//! forward, so two clones can never silently diverge on shared state. The only
//! cross-clone mutable object is the [`CancelSignal`].
//!
//! WHAT: [`StreamType`], [`IdempotencyLevel`], [`Spec`], [`Peer`],
//! [`RequestContext`], [`Ctx`], and [`CancelSignal`].
//!
//! **Extensions travel pathway (Decision 04):** HTTP middleware `&mut`-inserts on
//! the owned `SimpleIncomingRequest`; dispatch **moves** the map into
//! `RequestContext` (`take()`); post-construction writers use COW
//! `with_extension` — visibility is **downstream-only** (a layer holding an
//! earlier clone never sees a later insert), because each `Ctx` clone owns its
//! own extensions map (the `Arc`-valued entries are shared, the map is not).

mod cancel;

pub use cancel::CancelSignal;

use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_http::shared::context::ContextBag;
use foundation_netio::netcap::ConnectionContext;
use foundation_netio::shared::http::{Extensions, SimpleHeaders};

/// The RPC's streaming shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamType {
    /// One request, one response.
    Unary,
    /// A stream of requests, one response.
    ClientStream,
    /// One request, a stream of responses.
    ServerStream,
    /// Bidirectional streams.
    BidiStream,
}

/// How safe a procedure is to replay (drives HTTP GET eligibility / retries).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdempotencyLevel {
    /// Unknown — assume side effects.
    Unknown,
    /// No side effects — safe for HTTP GET.
    NoSideEffects,
    /// Idempotent — safe to retry.
    Idempotent,
}

/// Static description of a procedure.
#[derive(Debug, Clone)]
pub struct Spec {
    /// The RPC's streaming shape.
    pub stream_type: StreamType,
    /// Fully-qualified procedure, e.g. `"connectrpc.greet.v1.GreetService/Greet"`.
    pub procedure: String,
    /// True on the client side, false on the handler side.
    pub is_client: bool,
    /// Replay safety.
    pub idempotency: IdempotencyLevel,
}

impl Spec {
    /// An empty spec for a base/background context.
    #[must_use]
    pub fn empty(is_client: bool) -> Self {
        Self {
            stream_type: StreamType::Unary,
            procedure: String::new(),
            is_client,
            idempotency: IdempotencyLevel::Unknown,
        }
    }
}

/// The remote peer of a call.
#[derive(Debug, Clone)]
pub struct Peer {
    /// Remote address (`IP:port` on the server, `host:port` on the client).
    pub addr: String,
    /// Protocol name: `"connect"`, `"grpc"`, `"grpc-web"`.
    pub protocol: String,
}

impl Peer {
    /// An empty peer for a base/background context.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            addr: String::new(),
            protocol: String::new(),
        }
    }
}

/// Per-RPC state (owned, itself cheap-`Clone` with Arc'd internals). The shared
/// parts are immutable after construction; the only cross-clone mutable object
/// is the [`CancelSignal`].
#[derive(Clone)]
pub struct RequestContext {
    /// The procedure spec.
    pub spec: Arc<Spec>,
    /// The remote peer.
    pub peer: Arc<Peer>,
    /// Request headers.
    pub headers: Arc<SimpleHeaders>,
    /// Absolute deadline, if any (Copy).
    pub deadline: Option<Instant>,
    /// Type-map for untyped user/middleware data — Arc-valued, itself cheap-Clone
    /// (Decision 12 §13); each `Ctx` clone owns its own map.
    pub extensions: Extensions,
    /// Connection-scoped metadata (peer, TLS, ALPN, …) shared across the
    /// connection's requests (Decision 04 Q13 / Decision 12 §13).
    pub connection: Arc<ConnectionContext>,
    cancel: CancelSignal,
}

impl RequestContext {
    /// An empty per-call state (base/background context).
    #[must_use]
    pub fn empty(is_client: bool) -> Self {
        Self {
            spec: Arc::new(Spec::empty(is_client)),
            peer: Arc::new(Peer::empty()),
            headers: Arc::new(SimpleHeaders::new()),
            deadline: None,
            extensions: Extensions::new(),
            connection: Arc::new(ConnectionContext::default()),
            cancel: CancelSignal::new(),
        }
    }

    /// Assemble a per-call state from the parts the dispatcher resolved
    /// (Decision 08 dispatch flow / Decision 04 extensions travel pathway: the
    /// extensions map is **moved** in from the owned request). The transport holds
    /// a clone of `cancel` and fires it on RST/close/deadline.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn for_dispatch(
        spec: Spec,
        peer: Peer,
        headers: SimpleHeaders,
        deadline: Option<Instant>,
        extensions: Extensions,
        connection: Arc<ConnectionContext>,
        cancel: CancelSignal,
    ) -> Self {
        Self {
            spec: Arc::new(spec),
            peer: Arc::new(peer),
            headers: Arc::new(headers),
            deadline,
            extensions,
            connection,
            cancel,
        }
    }

    /// Whether the call has been canceled (sync poll — escape hatch).
    #[must_use]
    pub fn is_canceled(&self) -> bool {
        self.cancel.is_canceled()
    }

    /// Resolve once the call is canceled (race against unrelated work in `select!`).
    pub async fn cancelled(&self) {
        self.cancel.cancelled().await;
    }

    /// Time left until the deadline, if one is set.
    #[must_use]
    pub fn remaining_timeout(&self) -> Option<Duration> {
        self.deadline
            .map(|d| d.saturating_duration_since(Instant::now()))
    }

    /// The call's cancel signal — the transport holds a clone and fires it on
    /// RST/close/deadline.
    #[must_use]
    pub fn cancel_signal(&self) -> &CancelSignal {
        &self.cancel
    }
}

/// The context parameter every handler and client method receives, **by value,
/// in all four RPC kinds** (Decision 04). Owned + cheap-`Clone`.
#[derive(Clone)]
pub struct Ctx {
    /// App-scoped shared dependencies (DB pools, config, caches, …) — the same
    /// bag the HTTP layer hands `Serve` handlers.
    pub bag: Arc<ContextBag>,
    /// Per-RPC state.
    pub request: RequestContext,
}

impl Ctx {
    /// Client-side base context: app deps only, empty per-call state.
    #[must_use]
    pub fn client(bag: Arc<ContextBag>) -> Ctx {
        Ctx {
            bag,
            request: RequestContext::empty(true),
        }
    }

    /// Empty-bag base — tests, CLIs, wasm entry points.
    #[must_use]
    pub fn background() -> Ctx {
        Ctx {
            bag: Arc::new(ContextBag::new()),
            request: RequestContext::empty(false),
        }
    }

    // ── COW derivations: rebuild-and-move; the original is untouched ──

    /// Return a new `Ctx` with the given deadline (`now + d`). The original is
    /// unchanged; downstream layers receive the new context.
    #[must_use]
    pub fn with_deadline(mut self, d: Duration) -> Ctx {
        self.request.deadline = Some(Instant::now() + d);
        self
    }

    /// Return a new `Ctx` with an extension inserted. Visibility is
    /// **downstream-only**: a layer holding an earlier clone does not see it,
    /// because each `Ctx` clone owns its own extensions map.
    #[must_use]
    pub fn with_extension<T: Send + Sync + 'static>(mut self, v: T) -> Ctx {
        self.request.extensions.insert(v);
        self
    }

    /// Return a new `Ctx` with the cancel signal replaced — detach
    /// (`CancelSignal::new()`) or opt-in link (`CancelSignal::linked(&parent)`).
    /// Plain `clone()` always **shares** the current signal.
    #[must_use]
    pub fn with_cancellation(mut self, signal: CancelSignal) -> Ctx {
        self.request.cancel = signal;
        self
    }

    // ── Delegates for the common surface ──

    /// The procedure spec.
    #[must_use]
    pub fn spec(&self) -> &Spec {
        &self.request.spec
    }

    /// The remote peer.
    #[must_use]
    pub fn peer(&self) -> &Peer {
        &self.request.peer
    }

    /// Read an extension by type.
    #[must_use]
    pub fn extension<T: 'static>(&self) -> Option<&T> {
        self.request.extensions.get::<T>()
    }

    /// Whether the call has been canceled (sync poll).
    #[must_use]
    pub fn is_canceled(&self) -> bool {
        self.request.is_canceled()
    }

    /// Resolve once the call is canceled.
    pub async fn cancelled(&self) {
        self.request.cancelled().await;
    }

    /// Time left until the deadline, if any.
    #[must_use]
    pub fn remaining_timeout(&self) -> Option<Duration> {
        self.request.remaining_timeout()
    }

    /// The call's cancel signal.
    #[must_use]
    pub fn cancel_signal(&self) -> &CancelSignal {
        self.request.cancel_signal()
    }
}

impl core::fmt::Debug for Ctx {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ctx")
            .field("spec", &self.request.spec)
            .field("peer", &self.request.peer)
            .field("deadline", &self.request.deadline)
            .field("canceled", &self.request.is_canceled())
            .finish_non_exhaustive()
    }
}
