---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F00-crate-skeleton"
this_file: "specifications/52-tauri-foundation-platform/features/F00-crate-skeleton/feature.md"

status: pending
priority: critical
created: 2026-07-17

depends_on: []

tasks:
  completed: 0
  uncompleted: 22
  total: 22
  completion_percentage: 0%
---

# F00 — Crate scaffold and all shared types

## Overview

Create `foundation_platform` crate and define every shared type in
`foundation_ui_traits`. This feature owns ALL type definitions — every other
feature imports from here. The work splits into two clear halves:

1. **`foundation_ui_traits`** — Add a new module `platform_types.rs` with all
   pure-data enums and structs from [decision 02](../decisions/02-route-policy-model.md)
   (plus capability types from [decision 07](../decisions/07-native-capability-contract.md)
   and the Protocol enum from [decision 03](../decisions/03-session-backbone-transport.md)).
   The crate remains `#![no_std]` and dependency-free. All types must compile
   for `wasm32-unknown-unknown`.

2. **`foundation_platform`** — Flesh out the existing skeleton (`src/lib.rs` is
   empty, `Cargo.toml` has `tauri = "2.11.5"`). Wire full dependency set,
   implement `PlatformBuilder` that wraps `tauri::Builder`, and add
   `RouteDecision` convenience constructors.

[Decision 01](../decisions/01-platform-and-crates.md) defines the crate
boundaries and dependency graph.
[Decision 02](../decisions/02-route-policy-model.md) sections 3-4 define every
type. [Decision 03](../decisions/03-session-backbone-transport.md) section 5
defines the Protocol enum.

## Dependencies

None — this is the foundation.

Required by: every other feature (F01 needs the types, F02 needs the trait).

## What already exists

`foundation_ui_traits` exists from spec-39. Has `#![no_std]`, owns `DomOp`,
`Envelope`, `IntoHtml`, `Html`, `ProtocolEncoder`, `ColumnarEncoder`, etc.
We add a new module — no changes to existing code.

`foundation_platform` skeleton exists at `backends/foundation_platform/`.
`src/lib.rs` is empty. `Cargo.toml` has `tauri = "2.11.5"`, `serde`,
`serde_json`. We flesh it out — add dependencies, add `PlatformBuilder`.

---

## Part A — `foundation_ui_traits::platform_types` module

New file: `backends/foundation_ui_traits/src/platform_types.rs`

All types are pure data. No Tauri dependency, no WASM dependency, no I/O.
They compile for any Rust target including `wasm32-unknown-unknown`.

### A.1 — `NavigationIntent` struct

```rust
/// What the session backbone constructs from an intercepted navigation event.
/// Carries everything a route handler needs to make a decision.
#[derive(Debug, Clone)]
pub struct NavigationIntent {
    /// Full URL being navigated to (e.g. "ewe://localhost/app/items/42?proto=arrow")
    pub url: String,

    /// HTTP method semantics: GET (link click, initial load), POST (form submit),
    /// PUT/PATCH (programmatic navigation).
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
    /// User clicked a link (<a href>).
    LinkClick,
    /// User submitted a form (<form method="post">).
    FormSubmit,
    /// The server pushed a navigation (e.g. "go to /chat/room-5 now").
    ServerPush,
    /// A native gesture triggered navigation (swipe back, tab switch, deep link).
    NativeGesture,
    /// Programmatic navigation from application code.
    Programmatic,
}
```

**Why `url: String` not `Url`**: `foundation_ui_traits` is `no_std` and
dependency-free. The `url` crate pulls in `idna`, `percent-encoding`, etc.
URL parsing happens in `foundation_platform` where `std` is available. The
type system carries the semantic intent via the field name + doc comment.

### A.2 — `RouteDecision` struct

```rust
/// The output of a route handler. Pure data — the platform executes it.
/// Every field drives a specific step in the 9-step execution contract.
#[derive(Debug, Clone)]
pub struct RouteDecision {
    /// Where the content comes from.
    pub source: RouteSource,

    /// How the navigation is presented in the UI stack.
    pub presentation: Presentation,

    /// What kind of view renders this route: WebView or platform-native OS view.
    /// The platform does NOT need to know the content format (DomOps, HTML,
    /// Arrow, WASM module, etc.). That is foundation_wasm_ui's concern.
    pub view_kind: ViewKind,

    /// Preferred wire protocol for delivering the content.
    /// The backend can override this.
    pub protocol: ProtocolHint,

    /// Cache behavior for this route (offline strategy).
    pub cache_policy: CachePolicy,

    /// WebView trust profile for this route (gates platform services).
    /// Only enforced when view_kind is WebView; native views have their
    /// own OS-level trust model.
    pub profile: Profile,

    /// Identifies which registered native view component to instantiate.
    /// Only meaningful when view_kind is Native.
    pub native_view_id: Option<String>,

    /// Identifies which IPC target to talk to. Only meaningful when
    /// source is IpcShell. The session looks up this name in the wasm_app
    /// registry. Can also be set implicitly by route path auto-resolution.
    pub target: Option<String>,

    /// Native capabilities allowed on this route (per-route allowlisting).
    pub capabilities: Vec<CapabilityId>,
}
```

**10 fields, 5 concerns:**

| Fields | Concern |
|---|---|
| `source`, `target` | Routing — where content comes from |
| `presentation`, `view_kind`, `native_view_id` | Presentation — how it appears |
| `protocol` | Transport — wire format hint |
| `profile`, `capabilities` | Security — trust boundary |
| `cache_policy` | Caching — offline strategy |

Field-level semantics from [decision 02](../decisions/02-route-policy-model.md#the-routedecision-struct):

- `native_view_id` is meaningless when `source != IpcShell` AND `view_kind != Native`.
  The builder pattern makes it invisible in those cases — users never set it.
- `target` is meaningless when `source != IpcShell`. `ipc_shell()` and
  `ipc_shell_with("name")` set it; the default constructor leaves it `None`.
- Default `presentation` is `Morph` (in-place DOM patch, no stack change).
- Default `view_kind` is `WebView`.
- Default `protocol` is `Default` (backend decides).
- Default `cache_policy` is `NetworkFirst` (try network, fall back to cache).
- Default `profile` is determined by RouteSource (see A.8 below), not a fixed default.

### A.3 — `RouteSource` enum

```rust
/// Where content comes from. The platform selects the transport lane
/// automatically based on this value — the user never picks a transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteSource {
    /// App code generates content inside the WebView.
    /// No transport needed — the session signals the route change via
    /// postMessage and the in-WebView code renders directly.
    WebviewApp,

    /// Content comes from the native shell via IPC.
    /// Covers native Rust (.a/.so), WASM in wasmtime/wasmi, and
    /// Swift/Kotlin via the native bridge. All three are "call the
    /// shell process" from the platform's perspective.
    IpcShell,

    /// Content fetched from a remote server over the network.
    /// The session opens the best available transport: HTTP fetch,
    /// SSE stream, or WebSocket.
    RemoteServer,
}
```

**Transport implied by source** (session selects automatically):

| Source | Transport |
|---|---|
| `WebviewApp` | In-memory channel (no transport) |
| `IpcShell` | Tauri command IPC + shared memory for data |
| `RemoteServer` | HTTP/SSE/WebSocket via ewe:// custom protocol |

### A.4 — `Presentation` enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presentation {
    /// Replace the current screen's content in-place (default).
    /// The WebView's DOM is morphed/patched without a navigation transition.
    Morph,

    /// Push a new screen onto the navigation stack.
    /// Screenshot captured, new WebView created from pool, slide animation.
    Push,

    /// Present a modal screen over the current stack.
    /// Slides up from bottom (mobile) or appears as sheet/dialog (desktop).
    /// Modal has its own independent WebView.
    Modal,

    /// Replace the current screen in the stack (no new stack entry).
    /// Current screen is swapped; back button goes to the screen before it.
    Replace,

    /// Open in the system browser (external app).
    /// Platform hands URL to OS. Not rendered in-app.
    External,

    /// Clear the entire stack and set this as the new root.
    /// Used for login→main-app transitions or deep-link resets.
    Root,
}
```

Each variant is documented with its stack behavior in [decision 02](../decisions/02-route-policy-model.md#how-each-presentation-mode-works).

### A.5 — `ViewKind` enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    /// Content renders in a WebView (the default).
    /// foundation_wasm_ui handles ALL rendering — WASM apps, HTML docs,
    /// DomOps streams, fragment morphs, data projection, etc.
    /// The platform delivers bytes; the bootstrap script dispatches on
    /// Content-Type to the correct rendering pipeline.
    WebView,

    /// Content renders in a platform-native OS view.
    /// SwiftUI View on iOS, Jetpack Compose on Android, native widget
    /// on desktop. The native view component is registered with the
    /// platform's native view registry and identified by native_view_id.
    ///
    /// **Post-MVP.** The enum variant exists now so the type system
    /// doesn't change later, but the native view registry and
    /// instantiation code ship after the WebView rendering path is stable.
    /// MVP renders everything in WebViews.
    Native,
}
```

### A.6 — `ProtocolHint` enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolHint {
    /// Use the backend's native format. Whatever the backend produces is
    /// delivered as-is. This is the default.
    Default,

    /// Prefer columnar v1 encoding (DomOps batches, wasm-loop, no-std).
    Columnar,

    /// Prefer Arrow RecordBatch binary — raw columnar bytes cast to &[u8].
    /// Single batch, no streaming metadata. The default for data payloads.
    Arrow,

    /// Prefer Arrow IPC streaming format — Schema + DictionaryBatch +
    /// RecordBatch messages with continuation markers and EOS.
    /// For multi-batch streams and external interop (pyarrow, Flight).
    ArrowIpc,

    /// Prefer JSON encoding (debugging, interoperability).
    Json,

    /// Prefer HTML (server-rendered markup).
    Html,
}
```

### A.7 — `CachePolicy` enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachePolicy {
    /// Serve from cache if available; fetch from network only on cache miss.
    /// Offline: works (cached content).
    CacheFirst,

    /// Always try network first; fall back to cache on failure.
    /// Offline: serves stale cache.
    NetworkFirst,

    /// Never cache. Always fetch from the network.
    /// Offline: fails with offline error.
    OnlineOnly,

    /// Never fetch from the network. Always serve from cache.
    /// Offline: always works (no network needed).
    LocalOnly,

    /// Serve from cache immediately, then revalidate in background.
    /// Next navigation uses updated content. Offline: serves stale cache.
    StaleWhileRevalidate,
}
```

### A.8 — `Profile` enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Profile {
    /// Bundled local WASM, app shell HTML, platform UI components.
    /// Maximum trust — full platform access. ALL services allowed.
    App,

    /// Server-rendered HTML from the app's own backend, authenticated.
    /// High trust — scoped platform access.
    /// DB: read-only scoped queries. Auth: token-attached resources only.
    /// Native: route-allowlisted only. HTTP: allowed origins only.
    TrustedRemote,

    /// Third-party content, embedded pages, user-generated HTML.
    /// Minimal trust — sandboxed, no platform access.
    /// DB: none. Auth: none. Native: none. HTTP: same-origin fetch only.
    UntrustedRemote,

    /// Login screens, OAuth flows, credential entry.
    /// Elevated isolation — limited platform access.
    /// DB: none (auth session only). Native: biometric only.
    /// HTTP: auth provider origin only.
    Auth,

    /// Developer diagnostics, debug panels, hot-reload interfaces.
    /// Debug-only — full access, stripped from production builds.
    Devtools,
}
```

Deriving `PartialOrd` + `Ord` enables gate checks like `profile >= capability.min_profile()`
— higher-trust profiles inherit lower-trust profile permissions.

**Default profile assignment** (session applies when `RouteDecision` has no explicit profile):

| Route source | Default profile |
|---|---|
| Bundled WASM / local content | `App` |
| Remote, same origin as configured backend | `TrustedRemote` |
| Remote, different origin | `UntrustedRemote` |
| Auth path (`/auth/*` or configured) | `Auth` |

### A.9 — `CapabilityId`

```rust
/// Unique identifier for a native capability.
/// Globally registered, per-route allowlisted.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CapabilityId(pub String);

impl CapabilityId {
    pub const fn new(id: &'static str) -> Self {
        Self(id.to_string())
    }
}

// Well-known capability IDs:
impl CapabilityId {
    pub const CAMERA: Self = Self(String::new()); // initialized in const context elsewhere
    pub const MICROPHONE: Self = Self(String::new());
    pub const BIOMETRIC: Self = Self(String::new());
    pub const FILE_PICKER: Self = Self(String::new());
    pub const CLIPBOARD: Self = Self(String::new());
    pub const NOTIFICATIONS: Self = Self(String::new());
}
```

### A.10 — Session scoping types

```rust
/// Opaque session identifier. Generated at app launch.
/// Used for route scoping, page identity checks, stale-message guards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub u64);

/// Identifies a specific page load in a specific session.
/// Capability requests and responses carry this so results are
/// delivered to the right page. Stale-page guard checks this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageIdentity {
    pub session_id: SessionId,
    pub route: String,
    pub visit_id: u64,
}
```

### A.11 — Capability contract types

```rust
/// A request from the web side for a native capability.
/// Routed through the session backbone to the registered handler.
#[derive(Debug, Clone)]
pub struct CapabilityRequest {
    /// Unique request ID for matching response to caller.
    pub id: String,

    /// Which page made the request (for stale-page guard).
    pub page_identity: PageIdentity,

    /// The capability being invoked (e.g. "camera", "biometric_auth").
    pub capability: String,

    /// What action to perform (e.g. "capture", "authenticate").
    pub action: String,

    /// Action-specific parameters.
    pub payload: serde_json::Value,
}

/// The result of a capability invocation.
/// Delivered scoped to the requesting page.
#[derive(Debug, Clone)]
pub struct CapabilityResponse {
    /// Matches the request's id for correlation.
    pub id: String,

    /// Which page the response is for (must match request's page_identity).
    pub page_identity: PageIdentity,

    /// Ok(value) on success, Err(message) on failure.
    pub status: Result<serde_json::Value, String>,
}
```

**Crate-level note:** These types use `serde_json::Value` for payload flexibility.
`foundation_ui_traits` currently has NO dependency on `serde` or `serde_json`.
To keep the crate dependency-free:

**Option A (preferred):** Define `CapabilityRequest` and `CapabilityResponse`
in `foundation_platform` instead — they carry `serde_json::Value` which pulls
in `serde_json`, and those deps don't belong in a `no_std` crate. The
`PageIdentity` they reference can stay in `foundation_ui_traits` since it's
a pure-data struct with no serde requirement.

**Option B:** Add `serde` + `serde_json` features to `foundation_ui_traits`,
gated behind a `platform-types` feature flag that is only enabled when the
crate is used in a `std` context. This violates the "dependency-free" goal
but is pragmatic.

**Decision for F00:** Put `CapabilityRequest` and `CapabilityResponse` in
`foundation_platform`. They reference `PageIdentity` (which lives in
`foundation_ui_traits`) but the serde payloads don't belong in a `no_std`
crate. The route policy types (`NavigationIntent`, `RouteDecision`, all enums)
stay in `foundation_ui_traits` since they have zero dependencies.

### A.12 — `Protocol` enum

```rust
/// Wire protocol identifiers. Owned by foundation_wasm_ui's protocol
/// subsystem but defined here so foundation_platform can use them
/// for Content-Type header generation without depending on foundation_wasm_ui.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// DomOp batches — wasm-loop, no-std, TypedArray-friendly
    Columnar,

    /// Raw RecordBatch binary — single batch, no streaming metadata
    Arrow,

    /// Arrow IPC streaming format — multi-batch, Schema + EOS
    ArrowIpc,

    /// JSON encoding (debugging, interoperability)
    Json,

    /// Server-rendered HTML markup
    Html,

    /// Opaque binary payload
    CustomBinary,
}

impl Protocol {
    /// Map a `Protocol` variant to its HTTP Content-Type header value.
    pub fn content_type(&self) -> &'static str {
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
```

### A.13 — Module structure

```
foundation_ui_traits/src/
  ├── lib.rs              ← unchanged, existing spec-39 types
  ├── platform_types.rs   ← NEW — all types from this Part
  ├── dom_op.rs           ← existing
  ├── encoder.rs           ← existing
  ├── envelope.rs          ← existing
  └── ...                  ← existing
```

Add to `lib.rs`:
```rust
mod platform_types;
pub use platform_types::*;
```

### A.14 — Wasm target compatibility

All types MUST compile for `wasm32-unknown-unknown`. Verify with:
```bash
cargo check --package foundation_ui_traits --target wasm32-unknown-unknown
```

Key constraint: no `std` imports in `platform_types.rs`. `String` and `Vec`
come from `alloc` (already available via `#![no_std]` + `extern crate alloc`
at the crate root). `serde_json::Value` is NOT used here (see A.11 decision).

---

## Part B — `foundation_platform` crate build-out

### B.1 — `Cargo.toml` dependencies

```toml
[package]
name = "foundation_platform"
version = "0.0.1"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
keywords.workspace = true

[dependencies]
# Tauri — the host platform
tauri = { version = "2.11.5", features = [] }

# Shared types (dependency-free)
foundation_ui_traits = { path = "../foundation_ui_traits" }

# UI runtime (WASM-based DOM rendering, signals, templates)
foundation_wasm_ui = { path = "../foundation_wasm_ui" }

# Platform services (each crate does one thing)
foundation_db = { path = "../foundation_db" }
foundation_auth = { path = "../foundation_auth" }
foundation_nativeapis = { path = "../foundation_nativeapis" }
foundation_arrow = { path = "../foundation_arrow" }
foundation_signals = { path = "../foundation_signals" }
foundation_macros = { path = "../foundation_macros" }
foundation_wasmtime = { path = "../foundation_wasmtime" }
foundation_packager = { path = "../foundation_packager" }

# Standard deps
serde = { workspace = true }
serde_json = { workspace = true }
url = "2"                      # URL parsing in NavigationIntent construction
uuid = { version = "1", features = ["v4"] }  # CapabilityRequest IDs

[lints]
workspace = true
```

**Note:** `foundation_wasmtime` and `foundation_packager` may not exist yet —
they are new crates introduced in [decision 13](../decisions/13-wasm-app-entrypoint.md).
If they don't exist as crates, comment them out with a `# TODO: create before F11`
note and uncomment when those crates are scaffolded.

### B.2 — `PlatformBuilder`

```rust
// foundation_platform/src/lib.rs

mod builder;
mod types;  // CapabilityRequest, CapabilityResponse (see A.11)

pub use builder::PlatformBuilder;
pub use foundation_ui_traits::*;  // re-export all platform types
pub use types::*;

// foundation_platform/src/builder.rs

use tauri::App;

/// Wraps tauri::Builder. The user calls PlatformBuilder::new() instead of
/// tauri::Builder::default(). The platform extends Tauri's lifecycle —
/// it doesn't replace or contain it.
pub struct PlatformBuilder {
    inner: tauri::Builder,
}

impl PlatformBuilder {
    /// Create a new platform builder. Internally wraps tauri::Builder::default().
    /// The user chains .build() to produce a tauri::App.
    pub fn new() -> Self {
        Self {
            inner: tauri::Builder::default(),
        }
    }

    /// Consume the builder and produce a Tauri App.
    /// In future features, this will register:
    ///   - setup() hook → PlatformSession::initialize()
    ///   - ewe:// UriSchemeProtocol handler
    ///   - on_navigation() interception
    /// For F00, this is a pass-through — the skeleton compiles, nothing else wired.
    pub fn build(self) -> tauri::App {
        self.inner.build()
    }

    /// Access the inner tauri::Builder for advanced customization.
    /// Callers can register plugins, commands, menus, etc. directly on Tauri.
    pub fn inner_mut(&mut self) -> &mut tauri::Builder {
        &mut self.inner
    }
}

impl Default for PlatformBuilder {
    fn default() -> Self {
        Self::new()
    }
}
```

### B.3 — `CapabilityRequest` / `CapabilityResponse` in `foundation_platform`

```rust
// foundation_platform/src/types.rs

use foundation_ui_traits::PageIdentity;
use serde::{Deserialize, Serialize};

/// A request from the web side for a native capability.
/// Lives in foundation_platform (not foundation_ui_traits) because
/// it carries serde_json::Value — a heavy dep for a no_std crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    /// Unique request ID for matching response to caller.
    pub id: String,

    /// Which page made the request (for stale-page guard).
    pub page_identity: PageIdentity,

    /// The capability being invoked (e.g. "camera", "biometric_auth").
    pub capability: String,

    /// What action to perform (e.g. "capture", "authenticate").
    pub action: String,

    /// Action-specific parameters.
    pub payload: serde_json::Value,
}

/// The result of a capability invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityResponse {
    /// Matches the request's id for correlation.
    pub id: String,

    /// Which page the response is for.
    pub page_identity: PageIdentity,

    /// Ok(value) on success, Err(message) on failure.
    pub status: Result<serde_json::Value, String>,
}
```

### B.4 — `RouteDecision` convenience constructors

```rust
// foundation_platform/src/route.rs  (new file)

use foundation_ui_traits::*;

impl RouteDecision {
    /// Content generated by app code running inside the WebView.
    /// source=WebviewApp, view_kind=WebView, profile=App, cache=CacheFirst
    pub fn webview_app() -> Self {
        Self {
            source: RouteSource::WebviewApp,
            presentation: Presentation::Morph,
            view_kind: ViewKind::WebView,
            protocol: ProtocolHint::Default,
            cache_policy: CachePolicy::CacheFirst,
            profile: Profile::App,
            native_view_id: None,
            target: None,
            capabilities: Vec::new(),
        }
    }

    /// Content from the native shell. Default target (native shell process).
    pub fn ipc_shell() -> Self {
        Self {
            source: RouteSource::IpcShell,
            presentation: Presentation::Morph,
            view_kind: ViewKind::WebView,
            protocol: ProtocolHint::Default,
            cache_policy: CachePolicy::NetworkFirst,
            profile: Profile::TrustedRemote,
            native_view_id: None,
            target: None,
            capabilities: Vec::new(),
        }
    }

    /// Content from the native shell, explicitly targeting a named wasm_app instance.
    pub fn ipc_shell_with(target: impl Into<String>) -> Self {
        Self {
            target: Some(target.into()),
            ..Self::ipc_shell()
        }
    }

    /// Content fetched from a remote server.
    pub fn remote_fetch() -> Self {
        Self {
            source: RouteSource::RemoteServer,
            presentation: Presentation::Morph,
            view_kind: ViewKind::WebView,
            protocol: ProtocolHint::Default,
            cache_policy: CachePolicy::NetworkFirst,
            profile: Profile::TrustedRemote,
            native_view_id: None,
            target: None,
            capabilities: Vec::new(),
        }
    }
}

// Builder methods on RouteDecision — chain after constructor:

impl RouteDecision {
    pub fn with_presentation(mut self, p: Presentation) -> Self { self.presentation = p; self }
    pub fn with_view_kind(mut self, v: ViewKind) -> Self { self.view_kind = v; self }
    pub fn with_protocol(mut self, p: ProtocolHint) -> Self { self.protocol = p; self }
    pub fn with_cache_policy(mut self, c: CachePolicy) -> Self { self.cache_policy = c; self }
    pub fn with_profile(mut self, p: Profile) -> Self { self.profile = p; self }
    pub fn with_native_view(mut self, id: impl Into<String>) -> Self { self.native_view_id = Some(id.into()); self }
    pub fn with_target(mut self, target: impl Into<String>) -> Self { self.target = Some(target.into()); self }
    pub fn with_allowed_capabilities(mut self, caps: &[CapabilityId]) -> Self { self.capabilities = caps.to_vec(); self }
    pub fn with_auth_origin(mut self, _origin: &str) -> Self { self /* stored in session, not in decision */ }
}
```

**Builder chain example** (from decision 02):

```rust
session.route("/remote/dashboard", RouteDecision::remote_fetch()
    .with_presentation(Presentation::Push)
    .with_profile(Profile::TrustedRemote)
    .with_cache_policy(CachePolicy::NetworkFirst)
    .with_allowed_capabilities(&[CapabilityId("camera".into())]));
```

---

## Module structure after F00

```
backends/
├── foundation_ui_traits/
│   └── src/
│       ├── lib.rs               ← +pub mod platform_types; pub use platform_types::*;
│       ├── platform_types.rs    ← NEW — all types from Part A
│       └── ... (existing, unchanged)
│
└── foundation_platform/
    ├── Cargo.toml               ← updated with full dependency set
    └── src/
        ├── lib.rs               ← PlatformBuilder, CapabilityRequest/Response re-exports
        ├── builder.rs           ← PlatformBuilder struct
        ├── route.rs             ← RouteDecision constructors + builder methods
        └── types.rs             ← CapabilityRequest, CapabilityResponse
```

---

## Verification

### Type compilation
```bash
# All types compile for native target
cargo check --package foundation_ui_traits
cargo check --package foundation_platform

# All types compile for WASM target (critical — WASM UI uses these)
cargo check --package foundation_ui_traits --target wasm32-unknown-unknown
```

### Builder chain works
```bash
cargo test --package foundation_platform -- route_constructors
```

### Test: RouteDecision builder chain produces correct struct
```rust
#[test]
fn webview_app_sets_correct_defaults() {
    let d = RouteDecision::webview_app();
    assert_eq!(d.source, RouteSource::WebviewApp);
    assert_eq!(d.view_kind, ViewKind::WebView);
    assert_eq!(d.profile, Profile::App);
    assert_eq!(d.cache_policy, CachePolicy::CacheFirst);
    assert!(d.native_view_id.is_none());
    assert!(d.target.is_none());
}

#[test]
fn builder_chain_overrides_defaults() {
    let d = RouteDecision::remote_fetch()
        .with_presentation(Presentation::Push)
        .with_cache_policy(CachePolicy::CacheFirst)
        .with_profile(Profile::TrustedRemote)
        .with_allowed_capabilities(&[CapabilityId("camera".into())]);

    assert_eq!(d.source, RouteSource::RemoteServer);
    assert_eq!(d.presentation, Presentation::Push);
    assert_eq!(d.cache_policy, CachePolicy::CacheFirst);
    assert_eq!(d.profile, Profile::TrustedRemote);
    assert_eq!(d.capabilities.len(), 1);
}

#[test]
fn ipc_shell_with_sets_target() {
    let d = RouteDecision::ipc_shell_with("business_logic");
    assert_eq!(d.source, RouteSource::IpcShell);
    assert_eq!(d.target, Some("business_logic".to_string()));
}
```

### Test: Profile ordering enables gate checks
```rust
#[test]
fn profile_ordering() {
    assert!(Profile::App > Profile::TrustedRemote);
    assert!(Profile::TrustedRemote > Profile::UntrustedRemote);
    assert!(Profile::App > Profile::UntrustedRemote);
    // Auth is isolated — not directly comparable to App/TrustedRemote
}
```
