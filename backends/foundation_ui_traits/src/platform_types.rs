//! Platform types — pure data, no dependencies, compiles for any Rust target.
//!
//! These types are the shared vocabulary between `foundation_wasm_ui` (web side,
//! WASM) and `foundation_platform` (native side, Tauri). They carry route policy
//! decisions, navigation intents, session identity, and capability contracts.
//!
//! All types are `#[derive(Debug, Clone)]` — pure data, no behavior. The platform
//! crate executes the decisions these types represent.

use alloc::string::String;
use alloc::vec::Vec;

// ── Navigation intent ───────────────────────────────────────────────────────

/// What the session backbone constructs from an intercepted navigation event.
/// Carries everything a route handler needs to make a decision.
#[derive(Debug, Clone)]
pub struct NavigationIntent {
    /// Full URL being navigated to
    /// (e.g. "ewe://localhost/app/items/42?proto=arrow")
    pub url: String,

    /// HTTP method semantics: GET (link click, initial load), POST (form
    /// submit), PUT/PATCH (programmatic navigation).
    pub method: Method,

    /// What triggered this navigation.
    pub source: IntentSource,

    /// The URL of the page that initiated the navigation, if any.
    pub referrer: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentSource {
    /// User clicked a link (`<a href>`).
    LinkClick,
    /// User submitted a form (`<form method="post">`).
    FormSubmit,
    /// The server pushed a navigation (e.g. "go to /chat/room-5 now").
    ServerPush,
    /// A native gesture triggered navigation (swipe back, tab switch, deep
    /// link).
    NativeGesture,
    /// Programmatic navigation from application code.
    Programmatic,
}

// ── Route decision ──────────────────────────────────────────────────────────

/// The output of a route handler. Pure data — the platform executes it.
/// Every field drives a specific step in the 9-step execution contract.
#[derive(Debug, Clone)]
pub struct RouteDecision {
    /// Where the content comes from.
    pub source: RouteSource,

    /// How the navigation is presented in the UI stack.
    pub presentation: Presentation,

    /// What kind of view renders this route: WebView or platform-native OS
    /// view. The platform does NOT need to know the content format (DomOps,
    /// HTML, Arrow, WASM module, etc.). That is `foundation_wasm_ui`'s
    /// concern.
    pub view_kind: ViewKind,

    /// Preferred wire protocol for delivering the content.
    /// The backend can override this.
    pub protocol: ProtocolHint,

    /// Cache behavior for this route (offline strategy).
    pub cache_policy: CachePolicy,

    /// WebView trust profile for this route (gates platform services).
    /// Only enforced when `view_kind` is WebView; native views have their
    /// own OS-level trust model.
    pub profile: Profile,

    /// Identifies which registered native view component to instantiate.
    /// Only meaningful when `view_kind` is `Native`.
    pub native_view_id: Option<String>,

    /// Identifies which IPC target to talk to. Only meaningful when
    /// `source` is `IpcShell`. The session looks up this name in the
    /// wasm_app registry. Can also be set implicitly by route path
    /// auto-resolution (`/{name}/*` routes).
    pub target: Option<String>,

    /// Native capabilities allowed on this route (per-route allowlisting).
    pub capabilities: Vec<CapabilityId>,

    /// Auth origin for this route. Only meaningful when profile is Auth.
    pub auth_origin: Option<String>,

    /// Handler identity — the session looks up this name in the handler registry
    /// and delegates `respond()` to it. Set by the route constructor.
    pub handler_id: Option<String>,

    /// The path portion of the URL after stripping the matched route prefix.
    /// If the pattern is `/app/*` and the URL is `/app/v1/something`,
    /// this is `v1/something`. Set by the router during resolution.
    pub sub_path: Option<String>,
}

// ── Route source ────────────────────────────────────────────────────────────

/// Where content comes from. The platform selects the transport lane
/// automatically based on this value — the user never picks a transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteSource {
    /// App code generates content inside the WebView. No transport needed —
    /// the session signals the route change via `postMessage` and the
    /// in-WebView code renders directly.
    WebviewApp,

    /// Content comes from the native shell via IPC. Covers native Rust
    /// (`.a`/`.so`), WASM in wasmtime/wasmi, and Swift/Kotlin via the
    /// native bridge. All three are "call the shell process" from the
    /// platform's perspective.
    IpcShell,

    /// Content fetched from a remote server over the network. The session
    /// opens the best available transport: HTTP fetch, SSE stream, or
    /// WebSocket.
    RemoteServer,
}

// ── Presentation ────────────────────────────────────────────────────────────

/// How the navigation appears in the UI stack. Drives the WebView stack
/// manager ([decision 21](10-multi-webview-stack.md)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presentation {
    /// Replace the current screen's content in-place (default). The WebView's
    /// DOM is morphed/patched without a navigation transition.
    Morph,

    /// Push a new screen onto the navigation stack. Screenshot captured,
    /// new WebView created from pool, slide animation.
    Push,

    /// Present a modal screen over the current stack. Slides up from bottom
    /// (mobile) or appears as sheet/dialog (desktop). Modal has its own
    /// independent WebView.
    Modal,

    /// Replace the current screen in the stack (no new stack entry). Current
    /// screen is swapped; back button goes to the screen before it.
    Replace,

    /// Open in the system browser (external app). Platform hands the URL to
    /// the OS. Not rendered in-app.
    External,

    /// Clear the entire stack and set this as the new root. Used for
    /// login→main-app transitions or deep-link resets.
    Root,
}

// ── View kind ───────────────────────────────────────────────────────────────

/// What kind of view renders this route. The platform does NOT need to know
/// the content format — that is `foundation_wasm_ui`'s concern inside the
/// WebView, or the native view component's concern on the native side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    /// Content renders in a WebView (the default). `foundation_wasm_ui`
    /// handles ALL rendering — the platform delivers bytes; the bootstrap
    /// script dispatches on Content-Type.
    WebView,

    /// Content renders in a platform-native OS view. SwiftUI `View` on iOS,
    /// Jetpack `Composable` on Android, native widget on desktop.
    ///
    /// **Post-MVP.** The enum variant exists now so the type system doesn't
    /// change later, but the native view registry and instantiation code
    /// ship after the WebView rendering path is stable. MVP renders
    /// everything in WebViews.
    Native,
}

// ── Protocol hint ───────────────────────────────────────────────────────────

/// Preferred wire protocol for delivering content. The route handler provides
/// a hint. The actual protocol is resolved by priority: route decision →
/// `proto` query param → backend native format → platform default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolHint {
    /// Use the backend's native format. Whatever the backend produces is
    /// delivered as-is. This is the default.
    Default,

    /// Prefer columnar v1 encoding (DomOps batches, wasm-loop, no-std).
    Columnar,

    /// Prefer Arrow RecordBatch binary — raw columnar bytes cast to `&[u8]`.
    /// Single batch, no streaming metadata. The default for data payloads.
    Arrow,

    /// Prefer Arrow IPC streaming format — Schema + DictionaryBatch +
    /// RecordBatch messages with continuation markers and EOS indicator.
    /// For multi-batch streams and external interop (pyarrow, Flight).
    ArrowIpc,

    /// Prefer JSON encoding (debugging, interoperability).
    Json,

    /// Prefer HTML (server-rendered markup).
    Html,
}

// ── Cache policy ────────────────────────────────────────────────────────────

/// Controls whether and how content is cached, and what happens when the
/// network is unavailable. Assigned per-route in `RouteDecision`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachePolicy {
    /// Serve from cache if available; fetch from network only on cache miss.
    /// Offline: works (cached content).
    CacheFirst,

    /// Always try the network first; fall back to cache on failure.
    /// Offline: serves stale cache.
    NetworkFirst,

    /// Never cache. Always fetch from the network.
    /// Offline: fails with offline error.
    OnlineOnly,

    /// Never fetch from the network. Always serve from cache.
    /// Offline: always works (no network needed).
    LocalOnly,

    /// Serve from cache immediately, then revalidate in the background.
    /// Next navigation uses updated content. Offline: serves stale cache.
    StaleWhileRevalidate,
}

// ── Profile ─────────────────────────────────────────────────────────────────

/// WebView trust profile for this route. Each profile gates which platform
/// services the content can access. The session backbone enforces this at
/// runtime — every service call checks the active profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Bundled local WASM, app shell HTML, platform UI components.
    /// Maximum trust — full platform access.
    App,

    /// Server-rendered HTML from the app's own backend, authenticated.
    /// High trust — scoped platform access.
    TrustedRemote,

    /// Third-party content, embedded pages, user-generated HTML.
    /// Minimal trust — sandboxed, no platform access.
    UntrustedRemote,

    /// Login screens, OAuth flows, credential entry.
    /// Elevated isolation — limited platform access.
    Auth,

    /// Developer diagnostics, debug panels, hot-reload interfaces.
    /// Debug-only — full access, stripped from production builds.
    Devtools,
}

// ── Capability identifier ───────────────────────────────────────────────────

/// Unique identifier for a native capability.
/// Globally registered, per-route allowlisted.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CapabilityId(pub String);

// ── Session scoping ─────────────────────────────────────────────────────────

/// Opaque session identifier. Generated at app launch. Used for route
/// scoping, page identity checks, and stale-message guards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub u64);

/// Identifies a specific page load within a session. Capability requests
/// and responses carry this so results are delivered to the right page.
/// The stale-page guard checks this identity before delivering results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageIdentity {
    /// The session this page belongs to.
    pub session_id: SessionId,

    /// The route that loaded this page.
    pub route: String,

    /// Monotonically incrementing visit counter. Incremented on every
    /// navigation. Stale-page guard compares this against the current
    /// active visit.
    pub visit_id: u64,
}

// ── Protocol ────────────────────────────────────────────────────────────────

/// Wire protocol identifiers. Used by the platform for Content-Type header
/// generation without depending on `foundation_wasm_ui`'s encoder subsystem.
/// The actual encode/decode logic lives in `foundation_wasm_ui`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// DomOp batches — wasm-loop, no-std, TypedArray-friendly.
    Columnar,

    /// Raw RecordBatch binary — single batch, no streaming metadata.
    Arrow,

    /// Arrow IPC streaming format — multi-batch, Schema + EOS.
    ArrowIpc,

    /// JSON encoding (debugging, interoperability).
    Json,

    /// Server-rendered HTML markup.
    Html,

    /// Opaque binary payload.
    CustomBinary,
}

impl Protocol {
    /// Map a `Protocol` variant to its HTTP Content-Type header value.
    #[must_use]
    pub const fn content_type(self) -> &'static str {
        match self {
            Protocol::Columnar => "application/primal-columnar",
            Protocol::Arrow => "application/primal-arrow",
            Protocol::ArrowIpc => "application/vnd.apache.arrow.stream",
            Protocol::Json => "application/primal-json",
            Protocol::Html => "text/html; charset=utf-8",
            Protocol::CustomBinary => "application/primal-binary",
        }
    }
}
