# 02 — Route policy: `RouteHandler` trait and the navigation execution contract

**Date:** 2026-07-04
**Status:** Resolved

## Decision

Route policy is code, not a config file. The platform intercepts every
navigation intent through the session backbone and delegates decisions to
user-provided handlers. Three API surfaces compose together, all implementing a
single `RouteHandler` trait.

## Table of Contents

1. [The `RouteHandler` trait](#the-routehandler-trait)
2. [Three API surfaces](#three-api-surfaces)
3. [The `RouteDecision` struct](#the-routedecision-struct)
4. [Execution contract: what happens after a decision](#execution-contract-what-happens-after-a-decision)
5. [How each `presentation` mode works](#how-each-presentation-mode-works)
6. [How `ViewKind` drives rendering surface selection](#how-viewkind-drives-rendering-surface-selection)
7. [How `source` drives protocol and transport selection](#how-source-drives-protocol-and-transport-selection)
8. [How `profile` gates platform service access](#how-profile-gates-platform-service-access)
9. [How `cache_policy` controls offline behavior](#how-cache_policy-controls-offline-behavior)
10. [How `capabilities` are enforced per-route](#how-capabilities-are-enforced-per-route)
11. [Where it lives: crate boundaries](#where-it-lives-crate-boundaries)
12. [Why code, not config](#why-code-not-config)
13. [Why three APIs](#why-three-apis)

---

## The `RouteHandler` trait

The core abstraction. Patterned after `foundation_http`'s `Serve` trait: trait
object, user implements, registers with the platform.

```rust
trait RouteHandler: Send + Sync + 'static {
    fn resolve(
        &self,
        intent: &NavigationIntent,
        session: &PlatformSession,
    ) -> Option<RouteDecision>;
}
```

Handlers return `Some(decision)` to claim a navigation, or `None` to fall
through to the next handler. This is the only hard requirement.

`NavigationIntent` carries everything the handler needs to make a decision:

```rust
struct NavigationIntent {
    /// Full URL being navigated to (e.g. "ewe://localhost/app/items/42?proto=arrow")
    url: Url,

    /// HTTP method semantics: GET (link click, initial load), POST (form submit),
    /// PUT/PATCH (programmatic navigation).
    method: Method,

    /// What triggered this navigation.
    source: IntentSource,

    /// The URL of the page that initiated the navigation, if any.
    referrer: Option<Url>,
}

enum IntentSource {
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

---

## Three API surfaces

All three implement `RouteHandler`. They differ in how the user expresses their
policy, not in what the platform does with it.

### A — `RouteHandler` trait impl (full control)

The user implements the trait directly. Use when route decisions need shared
logic across screens — database queries, auth state checks, session context
inspection, conditional branching.

```rust
struct MyAppRouter {
    db: DatabaseHandle,
    auth: AuthManager,
}

impl RouteHandler for MyAppRouter {
    fn resolve(
        &self,
        intent: &NavigationIntent,
        session: &PlatformSession,
    ) -> Option<RouteDecision> {
        let user = self.auth.current_user(session)?;

        if intent.url.path().starts_with("/app/") {
            // Local WASM renders the app shell.
            Some(RouteDecision::webview_app()
                .with_profile(Profile::App))
        } else if intent.url.path().starts_with("/remote/") {
            // Remote server streams content. Profile depends on user auth.
            let profile = if user.is_authenticated() {
                Profile::TrustedRemote
            } else {
                Profile::UntrustedRemote
            };
            Some(RouteDecision::remote_fetch()
                .with_profile(profile)
                .with_cache_policy(CachePolicy::NetworkFirst))
        } else if intent.url.path().starts_with("/auth/") {
            Some(RouteDecision::remote_fetch()
                .with_profile(Profile::Auth)
                .with_auth_origin("https://auth.myapp.com"))
        } else {
            // Fall through — platform default handles this.
            None
        }
    }
}
```

### B — Closure convenience (wraps A)

A `FnRouteHandler(F)` wrapper impls `RouteHandler` for any matching closure.
Use when the policy is simple enough to inline.

```rust
session.on_navigate(|intent, _session| {
    if intent.url.path().starts_with("/app/") {
        Some(RouteDecision::webview_app())
    } else {
        None
    }
});
```

### C — Declarative pattern table (wraps A)

A `PatternRouter` struct impls `RouteHandler` — iterates registered patterns,
returns the first match. Use for the common case where navigation intent maps
directly to a known destination.

```rust
session.route("/app/*", RouteDecision::webview_app());
session.route("/remote/*", RouteDecision::remote_fetch());
session.route("/auth/*", RouteDecision::remote_fetch()
    .with_profile(Profile::Auth));
```

Pattern syntax: `*` matches a single path segment, `**` matches any depth. The
order of registration is the priority order — first registered, first matched.

### How they compose

A, B, and C are all `RouteHandler` impls registered in the session's handler
chain. On navigation, the session iterates handlers in registration order; the
first `Some(decision)` wins. If no handler claims the navigation, the platform
default applies:

- Routes with no handler match: treated as external, opened in system browser.
- The user can configure a fallback handler instead of the platform default.

```rust
// Handlers fire in registration order. First match wins.
session.register_handler(MyDatabaseBackedRouter::new(db, auth));  // A
session.on_navigate(my_closure);                                   // B
session.route("/app/*", RouteDecision::webview_app());              // C
session.route("/remote/*", RouteDecision::remote_fetch());          // C
```

---

## The `RouteDecision` struct

This is the canonical definition. Every field drives a specific execution step.
The struct lives in `foundation_ui_traits` (per [decision 01](01-platform-and-crates.md));
the platform crate (`foundation_platform`) executes it.

```rust
struct RouteDecision {
    /// Where the content comes from.
    source: RouteSource,

    /// How the navigation is presented in the UI stack.
    presentation: Presentation,

    /// What kind of view renders this route: a WebView or a platform-native
    /// OS view (SwiftUI, Jetpack Compose, etc.). Replaces the old
    /// `RenderMode` — the platform does NOT need to know the content format
    /// (DomOps, HTML, Arrow, WASM module, etc.). That is foundation_wasm_ui's
    /// concern. The platform only needs to know which view container to create.
    view_kind: ViewKind,

    /// Preferred wire protocol for delivering the content.
    /// The backend can override this.
    protocol: ProtocolHint,

    /// Cache behavior for this route (offline strategy).
    cache_policy: CachePolicy,

    /// WebView trust profile for this route (gates platform services).
    /// Only enforced when view_kind is WebView; native views have their
    /// own OS-level trust model.
    profile: Profile,

    /// Identifies which registered native view component to instantiate.
    /// Only meaningful when view_kind is Native. The platform looks up
    /// the view factory by this ID in the native view registry.
    native_view_id: Option<String>,

    /// Native capabilities allowed on this route (per-route allowlisting).
    capabilities: Vec<CapabilityId>,
}
```

### `RouteSource` — where content comes from

```rust
enum RouteSource {
    /// Content is generated by the app code running inside the WebView.
    /// The app module (WASM or the bootstrap script) handles this route
    /// directly — no external query needed. The platform signals the
    /// route change; the in-WebView code renders to the page.
    WebviewApp,

    /// Content is generated by a native shell process on the device.
    /// This covers BOTH paths: native Rust compiled as a static library
    /// (.a/.so) running in-process, AND user WASM running in an embedded
    /// wasmtime/wasmi runtime inside the native shell. From the platform's
    /// perspective these are the same thing — an IPC call to the shell
    /// process that returns content.
    IpcShell,

    /// Content is fetched from a remote server over the network.
    /// The appropriate transport lane (HTTP, SSE, WebSocket) carries it.
    RemoteServer,
}
```

**Why the rename from `LocalWasm` to `WebviewApp`:**

The old name `LocalWasm` was ambiguous. "Local WASM" could mean:
- WASM running **inside the WebView** (the `foundation-wasm-ui` runtime, or a
  user WASM module instantiated by the bootstrap script).
- WASM running **in the native shell** via wasmtime/wasmi — also local, also
  WASM, but a completely different execution context.

The distinction that matters to the platform is **where the code runs**, not
whether it's compiled to WASM:

| Source | Where code runs | Platform's job |
|---|---|---|
| `WebviewApp` | Inside the WebView | Signal route change, deliver bytes if fetching, step back |
| `IpcShell` | In the native process (any form: native Rust, WASM-in-wasmtime, Swift/Kotlin bridge) | IPC call to shell, deliver response bytes to WebView |
| `RemoteServer` | On a remote server | Open transport lane, deliver streamed response to WebView |

Convenience constructors:

```rust
RouteDecision::webview_app()   // RouteSource::WebviewApp
RouteDecision::ipc_shell()     // RouteSource::IpcShell
RouteDecision::remote_fetch()  // RouteSource::RemoteServer
```

**Cache is not a source.** `CachePolicy` on the `RouteDecision` already controls
*when* the cache is consulted (Step 3 of the execution contract). The cache wraps
the existing three sources — it can serve cached `WebviewApp` output, cached
`IpcShell` responses, or cached `RemoteServer` content. Making it a peer source
created a redundant code path that was never reached (the cache check in Step 3
always short-circuits before Step 5 resolves the source).

### `Presentation` — how navigation appears in the UI stack

```rust
enum Presentation {
    /// Replace the current screen's content in-place (default).
    /// The WebView's DOM is morphed/patched without a navigation transition.
    Morph,

    /// Push a new screen onto the navigation stack.
    /// Native-feeling slide animation (mobile) or instant (desktop).
    Push,

    /// Present a modal screen over the current stack.
    /// Slides up from bottom (mobile) or appears as a sheet/dialog (desktop).
    Modal,

    /// Replace the current screen in the stack (no new stack entry).
    /// The current screen is swapped; back button goes to the screen before it.
    Replace,

    /// Open in the system browser (external app).
    /// The platform hands the URL to the OS. Not rendered in-app.
    External,

    /// Clear the entire stack and set this as the new root.
    /// Used for login→main-app transitions or deep-link resets.
    Root,
}
```

The distinction between `presentation` and `view_kind`:

- **`presentation`** controls the **navigation stack** — where the screen lives
  in the stack hierarchy, what transition plays, how the back button behaves.
  It drives the WebView stack manager ([decision 21](10-multi-webview-stack.md)).

- **`view_kind`** controls the **rendering surface** — whether the platform
  creates a WebView or a platform-native OS view (SwiftUI `View`, Jetpack
  `Composable`, etc.) for this route. The platform does NOT need to know what
  format the content is in (DomOps, HTML, Arrow, WASM module, etc.). That is
  `foundation_wasm_ui`'s concern inside the WebView, or the native view
  component's concern on the native side.

These are orthogonal. You can `Morph`-replace a `WebView` screen or
`Push`-navigate to a `Native` screen. The stack manager handles
presentation; `view_kind` determines which view container the platform creates.

### `ViewKind` — what kind of view renders this route

```rust
enum ViewKind {
    /// Content renders in a WebView (the default for most routes).
    /// foundation_wasm_ui handles all rendering concerns — WASM apps,
    /// HTML documents, DomOps streams, fragment morphs, data projection,
    /// etc. The platform does NOT need to know which rendering path
    /// the runtime takes. The platform's job is to deliver bytes;
    /// the bootstrap script (foundation-wasm-ui.js) interprets the
    /// Content-Type and routes to the correct rendering pipeline.
    WebView,

    /// Content renders in a platform-native OS view.
    /// The platform instantiates a registered native view component:
    /// SwiftUI View on iOS, Jetpack Compose on Android, native widget
    /// on desktop. The native view receives data through the IPC shell
    /// lane or capability bridge. The view component is registered
    /// with the platform's native view registry (see below).
    Native,
}
```

### Why `ViewKind` replaces `RenderMode`

The old `RenderMode` enum enumerated content formats (`WasmApp`, `HtmlDocument`,
`DomOpsStream`, `FragmentMorph`, `DataProjection`) as platform-level concerns.
This was wrong at two levels:

1. **The platform doesn't render anything.** It delivers bytes to a WebView or
   instantiates a native view. What happens inside the WebView after delivery is
   `foundation_wasm_ui`'s business. The runtime's bootstrap script
   (`foundation-wasm-ui.js`) receives the response, checks the `Content-Type`,
   and routes to the correct pipeline — WASM instantiation, DomOp decoding,
   HTML parsing, signal graph projection, etc. The platform never branches on
   content format.

2. **Content format is not a route-level decision.** The frontend code already
   expresses what it wants: `mount_stream()` expects DomOps,
   `mount_data()` expects Arrow/JSON, a `<script type="module">` tag loads WASM.
   The route handler doesn't need to duplicate this. The runtime inside the
   WebView knows what it asked for; let it handle the response format.

`ViewKind` captures the ONLY rendering decision the platform actually makes:
**WebView or native OS view?** Everything else is the runtime's concern.

### How Hotwire Native handles this

Hotwire Native separates the same concerns. Its `PathConfiguration` JSON (served
by the backend) maps URL patterns to rules. The key rules are:

| PathConfiguration key | What it controls | Our equivalent |
|---|---|---|
| `presentation` | How the screen appears in the nav stack (`push`, `replace`, `modal`, `pop`, `replace_root`, `clear_all`) | `Presentation` enum |
| Native view controller registration | Whether a URL pattern maps to a native `UIViewController` (iOS) / `Activity` (Android) or the `WKWebView` Turbo Session | `ViewKind` enum |
| `layout` | Which native chrome/layout wraps the screen | Native view component registration |
| `context` | Arbitrary data passed to the native view | Capability payload |

The critical insight from Hotwire Native: **the decision between native view and
WebView is made at the route level, by pattern-matching the URL.** If a URL
matches a registered native view controller pattern, the native VC is
instantiated and pushed onto the navigation stack. If no native VC matches, the
shared `WKWebView` handles it through Turbo.

Our model mirrors this, but in code rather than JSON:

```rust
// Route handler (the "PathConfiguration" — but code, not config):
session.route("/app/settings/*", RouteDecision::ipc_shell()
    .with_view_kind(ViewKind::Native)
    .with_native_view("settings"));
    // → Platform instantiates the registered "settings" native view
    //   (SwiftUI SettingsView on iOS, Compose SettingsScreen on Android).

session.route("/app/feed/*", RouteDecision::remote_fetch()
    .with_view_kind(ViewKind::WebView)
    .with_presentation(Presentation::Push));
    // → Platform creates a WebView, fetches from remote, delivers bytes.
    //   foundation_wasm_ui handles rendering (DomOps, HTML, whatever).

session.route("/app/camera/*", RouteDecision::webview_app()
    .with_view_kind(ViewKind::WebView));
    // → Local WASM module runs in the WebView. Platform bootstraps
    //   the WASM runtime and steps back.
```

### Native view registration

Native view components are registered with the platform, similar to how
capabilities are registered ([decision 18](07-native-capability-contract.md)):

```rust
// Register a native view component:
session.register_native_view(
    "settings",                              // view ID
    |intent, shell| -> NativeViewHandle {    // factory
        // On iOS: creates a SwiftUI SettingsView
        // On Android: creates a Compose SettingsScreen
        // On desktop: creates a native settings window/widget
        shell.create_native_view::<SettingsView>(intent)
    }
);
```

### What Tauri provides (verified against source)

Tauri is fundamentally a WebView host — its `Runtime` trait
(`tauri-runtime/src/lib.rs`) provides `create_window` and `create_webview` but
has **no built-in `create_native_view` API**. That's fine. Tauri provides
escape hatches that let `foundation_platform` build native view support on top:

**iOS** (`tao/src/platform_impl/ios/window.rs`):

Tao creates a `UIWindow` + `UIViewController` + `UIView` for each window:

```rust
// tao/src/platform_impl/ios/window.rs
pub struct Inner {
    pub window: id,           // UIWindow
    pub view_controller: id,  // UIViewController
    pub view: id,             // UIView
    gl_or_metal_backed: bool,
}
```

Tauri calls `on_webview_created(webview: *const c_void, controller: *const c_void)`
(`tauri/src/ios.rs`) via swift-rs, passing both the `WKWebView` and its parent
`UIViewController` to Swift. This means the native side already has access to
the view controller hierarchy.

Escape hatches we use:
- `WindowDispatch::run_on_main_thread()` (`tauri-runtime/src/lib.rs:306`) —
  run arbitrary code on the main thread to create/push native UIViewControllers.
- `RunEvent::SceneRequested { scene, options }` (`tauri-runtime/src/lib.rs:242`) —
  surfaces the `UIScene` when iOS requests a new scene (e.g., for a native view).
- `InputAccessoryViewBuilder` (`wry/src/lib.rs:420`) — allows custom UIKit
  `UIView` injection into the keyboard area (narrow, but demonstrates the
  pattern of mixing UIKit with WebView).

**Android** (`wry/src/android/mod.rs`, `tauri-runtime/src/lib.rs`):

- `RuntimeHandle::run_on_android_context()` — runs arbitrary code on the
  Android UI thread with `JNIEnv`, `Activity`, and `WebView` references.
  This is how we create native Jetpack Compose views or Fragments.
- `WindowDispatch::find_class()` / `activity_name()` — JNI class resolution
  and Activity identification.

**Desktop** (`wry/src/lib.rs`):

- `WebViewBuilder::build_as_child()` (line 1562) — creates a WebView as a child
  of a native window's content view. On macOS this is an `NSView` subview.
  Native views can be siblings in the same window's view tree.
- `WindowDispatch::create_window()` — creates a native OS window that can
  hold native content. The platform can create a native window (e.g., with a
  SwiftUI/GTK/WinUI surface) alongside WebView windows.

### How foundation_platform builds on this

The platform does not rely on Tauri having a native view abstraction. Instead:

1. **Native view factory registration** — `register_native_view()` stores a
   closure in the platform's native view registry (similar to the capability
   registry).

2. **Instantiation via thread hooks** — When a route resolves to `ViewKind::Native`,
   the platform calls `run_on_main_thread` (or the Android equivalent) and
   runs the registered factory closure. On iOS, this creates a
   `UIViewController` (or `UIHostingController` for SwiftUI) and pushes it
   onto the `UINavigationController`. On Android, it creates a `Fragment`
   or Compose view through JNI.

3. **Integration with the stack manager** — The `ScreenSlot` enum (defined in
   [How `ViewKind` drives rendering surface selection](#how-viewkind-drives-rendering-surface-selection))
   holds either a `WebView` or a `NativeViewHandle`. Native views participate
   in the same push/pop/morph/present semantics as WebView screens.

4. **Communication through the session backbone** — Native views don't talk
   to the platform directly. They use the same capability bridge as WebView
   screens: `CapabilityRequest` → session backbone → `CapabilityResponse`.
   This is identical to how Hotwire Native's bridge components work — native
   views receive data through the session backbone, not ad-hoc channels.

**Tauri does not provide the native view abstraction. foundation_platform builds it using Tauri's escape hatches.** This is the same approach as the
capability registry: Tauri provides the raw primitives (window management,
WebView creation, thread hooks, JNI access); the platform builds the
coordination layer.

### Data flow for native views

When `view_kind` is `Native`, the platform:
1. Instantiates the registered native view component via `run_on_main_thread`
2. Resolves the `RouteSource` to get data:
   - `WebviewApp` → not typically used with `Native` view_kind (code
     runs inside the WebView, not applicable to native views)
   - `IpcShell` → native shell generates data directly for the native view
     (either native Rust, or WASM running in wasmtime/wasmi inside the shell)
   - `RemoteServer` → platform fetches from remote, passes the response to
     the native view
3. The native view receives data through the capability bridge
4. The native view owns its own rendering — the platform steps back

### `ProtocolHint` — preferred wire format

```rust
enum ProtocolHint {
    /// Use the backend's native format. Whatever the backend produces is
    /// delivered as-is. This is the default.
    Default,

    /// Prefer columnar v1 encoding (DomOps batches, wasm-loop, no-std).
    Columnar,

    /// Prefer Arrow RecordBatch binary — raw columnar bytes cast to &[u8].
    /// Single batch, no streaming metadata. Arrow's in-memory layout IS the
    /// wire format. The default Arrow protocol for most use cases.
    Arrow,

    /// Prefer Arrow IPC streaming format — Schema + DictionaryBatch +
    /// RecordBatch messages with continuation markers and EOS indicator.
    /// For multi-batch data streams where the schema may change.
    /// Interoperable with the Apache Arrow ecosystem (pyarrow, Flight, etc.).
    ArrowIpc,

    /// Prefer JSON encoding (debugging, interoperability).
    Json,

    /// Prefer HTML (server-rendered markup).
    Html,
}
```

The route handler provides a hint. The actual protocol is resolved by priority
(per [decision 03](03-session-backbone-transport.md)):

1. Route handler's `RouteDecision.protocol` — user's explicit choice.
2. `proto` query parameter on the URL — WebView requests a specific format.
3. Backend's native format — whatever the backend produces. If WASM emits Arrow
   IPC, that's what gets delivered.
4. Platform default — columnar v1 for DomOps, Arrow binary for data payloads.

### `CachePolicy` — offline strategy

```rust
enum CachePolicy {
    /// Serve from cache if available; fetch from network only if cache miss.
    /// Best for static content, offline-first apps.
    CacheFirst,

    /// Always try the network first; fall back to cache on failure.
    /// Best for dynamic content that should be fresh when possible.
    NetworkFirst,

    /// Never cache. Always fetch from the network.
    /// Best for real-time data, auth-gated content.
    OnlineOnly,

    /// Never fetch from the network. Always serve from cache.
    /// Best for bundled content that never changes.
    LocalOnly,

    /// Serve from cache immediately, then revalidate in the background.
    /// On next navigation to this route, the updated content is used.
    /// Best for content that should feel instant but stay fresh.
    StaleWhileRevalidate,
}
```

### `Profile` — WebView trust boundary

```rust
enum Profile {
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
```

The profile is assigned by the route handler in the `RouteDecision`. If no
profile is specified, the platform defaults apply:

| Route source | Default profile |
|---|---|
| Bundled WASM / local content | `App` |
| Remote, same origin as configured backend | `TrustedRemote` |
| Remote, different origin | `UntrustedRemote` |
| Auth path (`/auth/*` or configured) | `Auth` |

Full profile details (service access gates, CSP, cross-profile isolation) are
in [decision 14](06-webview-profiles.md). The profile is enforced by the session
backbone at runtime — every platform service call checks the active profile
before allowing access.

### `capabilities` — per-route native capability allowlisting

The route handler declares which native capabilities are allowed on this
specific route. Even if a capability is globally registered, it is blocked on
this route unless it appears in this list.

```rust
session.route("/remote/video/*", RouteDecision::remote_fetch()
    .with_profile(Profile::TrustedRemote)
    .with_allowed_capabilities(&[
        CapabilityId::camera,
        CapabilityId::microphone,
    ]));

// /remote/content/* with no capabilities listed → all capability requests denied,
// even if the camera is globally registered.
session.route("/remote/content/*", RouteDecision::remote_fetch());
```

At runtime, the session checks: is this capability in the route's allowlist?
If not, the request is denied before it reaches the capability handler. This
is a defense-in-depth layer on top of the profile's broader service gates.

---

## Execution contract: what happens after a decision

When a user taps a link, submits a form, or triggers any navigation, this is
the full execution chain:

### Step 1: Navigation interception

The session backbone intercepts the navigation intent. The interception point
depends on the source:

- **In-WebView link click / form submit:** Tauri's `on_navigation()` callback
  fires. The session backbone receives the URL, method, and referrer. It
  constructs a `NavigationIntent` with `IntentSource::LinkClick` or
  `IntentSource::FormSubmit`.
- **Server push:** The transport lane (SSE, WebSocket, or `ewe://` custom
  protocol) receives a navigation command from the backend. The session
  backbone constructs a `NavigationIntent` with `IntentSource::ServerPush`.
- **Native gesture:** iOS `UINavigationController` swipe-back or Android back
  button. The platform's native bridge translates it to a navigation pop. The
  session backbone constructs a `NavigationIntent` with
  `IntentSource::NativeGesture`.
- **Deep link / URL scheme:** The OS hands a URL to the app. The platform
  shell receives it, routes it through the session backbone.

### Step 2: Route handler chain

The session iterates its registered `RouteHandler`s in registration order:

```rust
// Inside PlatformSession::resolve_route():
fn resolve_route(&self, intent: &NavigationIntent) -> RouteDecision {
    for handler in &self.route_handlers {
        if let Some(decision) = handler.resolve(intent, self) {
            return decision;  // first match wins
        }
    }
    // No handler claimed it — platform default.
    RouteDecision::default_for(intent)
}
```

The platform default: if the URL is to a known external origin, open in system
browser (`Presentation::External`). If it's same-origin with no handler,
render in a WebView with `UntrustedRemote` profile. The user can override
the default by registering a catch-all handler last in the chain.

### Step 3: Cache check

Before reaching the backend, the session checks the cache policy:

```rust
match decision.cache_policy {
    CachePolicy::CacheFirst | CachePolicy::LocalOnly => {
        if let Some(cached) = self.cache.get(&intent.url) {
            // Serve from cache immediately. The backend is not queried.
            return self.render_cached(cached, &decision);
        }
        // Cache miss — fall through to backend query.
    }
    CachePolicy::StaleWhileRevalidate => {
        if let Some(cached) = self.cache.get(&intent.url) {
            // Serve from cache now, trigger background revalidation.
            self.spawn_background_revalidate(&intent.url, &decision);
            return self.render_cached(cached, &decision);
        }
    }
    CachePolicy::NetworkFirst | CachePolicy::OnlineOnly => {
        // Always go to backend. Cache is not checked.
    }
}
```

### Step 4: Presentation execution

The session hands the `RouteDecision.presentation` to the WebView stack
manager ([decision 21](10-multi-webview-stack.md)):

```
Presentation::Morph    → stack manager stays on current screen,
                         rendering lane patches the DOM in-place

Presentation::Push     → stack manager creates/assigns a new WebView,
                         captures screenshot of current screen,
                         animates transition (slide on mobile, instant on desktop),
                         loads new route into the new WebView

Presentation::Modal    → stack manager creates a modal WebView,
                         slides up from bottom (mobile) or shows sheet (desktop),
                         modal has its own independent WebView

Presentation::Replace  → stack manager swaps current screen's WebView
                         with new content, no new stack entry

Presentation::External → platform hands URL to OS (system browser, in-app browser,
                         or share sheet depending on platform)

Presentation::Root     → stack manager clears all screens, sets this as root,
                         used for login→main-app transitions
```

The stack manager's job: manage WebView lifecycle (create, pool, screenshot,
show, hide, destroy). The rendering lane's job: turn content into pixels.

### Step 5: Backend query (source resolution)

Depending on `RouteDecision.source`, the session queries the appropriate
backend:

```rust
match decision.source {
    RouteSource::WebviewApp => {
        // The app code is already running inside the WebView.
        // The session signals the in-WebView code to handle this route
        // (via postMessage or equivalent). No IPC, no network — the code
        // in the WebView renders directly. The platform steps back.
    }
    RouteSource::IpcShell => {
        // Content comes from the native shell via IPC.
        // This covers native Rust (.a/.so), WASM running in wasmtime/wasmi
        // embedded in the shell, and Swift/Kotlin code via the native bridge.
        // The session sends a request over the Tauri command IPC lane
        // or the native shell IPC lane (shared memory Arrow).
        // The native shell processes the request and returns content.
    }
    RouteSource::RemoteServer => {
        // Content comes from a remote server.
        // The session opens a transport: HTTP fetch, SSE stream, WebSocket.
        // The transport lane carries the request and streams the response.
        // Auth tokens are attached by the shell (never enter JS context).
    }
}
```

**`WebviewApp` does not "query" a backend.** The code is already running in the
WebView. The session signals the route change; the in-WebView code handles
rendering. This is different from `IpcShell` (IPC call to native process) and
`RemoteServer` (network call to remote). The old `LocalWasm` name obscured this
by suggesting the WASM was "local" in a general sense — it's specifically
*inside the WebView*.

Cache is not a source — `CachePolicy` was already checked in Step 3. If we
reach Step 5, the cache either missed or the policy said "go to backend."
The session queries the actual source (`WebviewApp`, `IpcShell`, or
`RemoteServer`) directly.

### Step 6: Protocol selection

The session selects the wire protocol for the response (see [How `source`
drives protocol and transport selection](#how-source-drives-protocol-and-transport-selection)):

1. `RouteDecision.protocol` — explicit user choice.
2. `proto` query parameter on the URL.
3. Backend's native output format.
4. Platform default.

### Step 7: Content encoding and transport

The platform encodes the backend's output in the selected protocol and
delivers it through the appropriate transport lane:

- **`ewe://` (default):** Tauri custom protocol — binary response with typed
  `Content-Type`. The `UriSchemeProtocol` handler parses the URI, routes
  through the session, encodes the response, and responds.
- **`ewe+ipc://`:** Tauri command IPC — JSON for control messages, or Arrow
  binary via `InvokeBody::Raw(Vec<u8>)` (desktop+iOS only, not Android).
  Control lane for typed request/response. For streaming data or Android
  Arrow payloads, use the custom protocol lane.
- **`ewe+ws://`:** WebSocket — bidirectional streaming for live DomOps,
  collaborative editing, real-time sync.
- **`ewe+http://`:** HTTP fetch — standard web semantics, proxies to dev
  server or remote.

### Step 8: View instantiation

The platform creates the rendering surface determined by `RouteDecision.view_kind`:

```
ViewKind::WebView → Platform acquires a WebView from the pool (or creates one).
                    Content bytes are delivered through the transport lane.
                    foundation-wasm-ui.js bootstrap script receives the response,
                    checks Content-Type, and routes to the correct rendering
                    pipeline (WASM instantiation, DomOp decoding, HTML parsing,
                    signal graph projection, etc.). The platform steps back —
                    it does not interpret the content format.

ViewKind::Native  → Platform looks up the registered native view component by ID.
                    Instantiates the native OS view (SwiftUI View, Jetpack
                    Compose, etc.) through the platform's native bridge.
                    Passes the NavigationIntent and initial data payload.
                    The native view owns its rendering surface. The platform
                    steps back — the native view communicates through the
                    capability bridge for subsequent data/events.
```

Full details in [How `ViewKind` drives rendering surface
selection](#how-viewkind-drives-rendering-surface-selection).

### Step 9: Post-render

- The session records the navigation in its history.
- The stack manager updates slot state (`Active`, `Ready`, `Screenshot`, etc.).
- The session emits a `navigated` event — any subsystem (cache, analytics,
  capability registry) can react.
- The previous screen's screenshot is captured and stored in its slot.
- If the new screen has a preloaded WebView waiting, it becomes active.
- If the new screen is stale (`isShowingStaleContent`), a reload is triggered.

### End-to-end trace: user clicks a link in a WASM app

Concrete example of the full chain:

```
1. User clicks <a href="/remote/dashboard"> in a local WASM screen.

2. Tauri's on_navigation() fires.
   → Session constructs NavigationIntent {
       url: "ewe://localhost/remote/dashboard",
       method: GET,
       source: LinkClick,
       referrer: Some("ewe://localhost/app/home"),
     }

3. Route handler chain runs:
   - Handler 1 (PatternRouter): "/app/*" → no match
   - Handler 2 (PatternRouter): "/remote/*" → matches!
     Returns RouteDecision {
       source: RemoteServer,
       presentation: Push,
       view_kind: WebView,
       protocol: Default,
       cache_policy: NetworkFirst,
       profile: TrustedRemote,
       capabilities: vec![CapabilityId::camera],
     }

4. Cache check: NetworkFirst → skip cache, go to backend.

5. Presentation: Push.
   Stack manager captures screenshot of current screen.
   Creates a new WebView from pool.
   Animates slide transition.

6. Source: RemoteServer.
   Session opens an HTTP connection to https://api.myapp.com/remote/dashboard.
   Auth token attached by shell (from secure storage, never enters JS).

7. Backend responds with a stream of binary columnar v1 DomOps.

8. Protocol: Default → backend's native format (columnar v1) is used.

9. Transport: response delivered through ewe:// custom protocol as
   application/primal-columnar ArrayBuffer.

10. View: WebView.
    Platform acquires WebView, delivers bytes.
    foundation-wasm-ui.js bootstrap script receives the stream.
    Content-Type is application/primal-columnar → runtime decodes DomOp batches,
    applies them to the new screen's DOM. Screen renders incrementally.
    The platform never branches on content format — the bootstrap script
    handles the routing.

11. Post-render:
    Session records navigation in history.
    Stack manager sets new slot to Active, old slot to Screenshot.
    Session emits navigated event.
```

### End-to-end trace: user taps back (native gesture)

```
1. User swipes from left edge (iOS) or taps back button (Android).

2. Native bridge translates gesture to NavigationIntent {
     url: "ewe://localhost/app/home",  // the referrer of the pushed screen
     method: GET,
     source: NativeGesture,
     referrer: None,
   }

3. Route handler chain runs — same route, may return different decision
   (e.g. cache-first for back navigation, since the content was already seen).

4. Cache check: CacheFirst.
   Cache has "/app/home" → serve from cache instantly.

5. Presentation: the stack manager pops the top screen.
   Previous screen's screenshot is shown instantly.
   WebView becomes active.
   Cached content renders immediately (no network wait).

6. If the cached content is stale, the session triggers a reload
   AFTER the WebView is visible — the user sees the stale content briefly,
   then it updates. No blank screen, no spinner.
```

---

## How each `presentation` mode works

### `Morph` (default)

No stack change. The current WebView stays active. The rendering lane patches
the existing DOM — DomOps are applied in-place, an HTML fragment is morphed
in, new data is projected through signals. No transition animation. No
screenshot. The URL in the address bar updates.

Use for: same-screen interactions (filtering a list, paginating, updating a
counter), tab switches within a screen, live updates from a DomOps stream.

### `Push`

New screen on the navigation stack. The stack manager:

1. Captures a screenshot of the current WebView.
2. Stores it in the current slot. Slot state: `Active → Screenshot`.
3. Acquires a WebView for the new screen (from pool or new).
4. Positions the new WebView off-screen (right on mobile, not visible on
   desktop).
5. Animates the transition: old screenshot slides left, new WebView slides in
   from right (mobile). On desktop, instant swap unless animations are enabled.
6. New slot state: `Active`. Old slot: `Screenshot`.
7. Loads the new route into the new WebView.
8. The back button/gesture now pops back to the previous screen.

Use for: standard navigation (list → detail, menu → content page), any
drill-down interaction.

### `Modal`

A new screen presented over the current stack. The stack manager:

1. Creates a modal WebView (separate from the main stack's WebViews).
2. Animates it sliding up from the bottom (mobile) or appearing as a centered
   sheet/dialog (desktop).
3. The main stack is still there underneath — it's not destroyed, just
   obscured.
4. Modal has its own independent navigation (if the user navigates within the
   modal).
5. Dismiss: swipe down (mobile), close button (desktop), or programmatic
   dismiss. The modal WebView is returned to pool or destroyed.

Use for: forms, settings panels, image pickers, any self-contained flow that
shouldn't pollute the main navigation stack.

### `Replace`

Swap the current screen without adding to the stack. The stack manager:

1. Captures a screenshot of the current WebView (for the back transition, if
   the user navigates back).
2. Unloads the current route from the WebView.
3. Loads the new route into the SAME WebView (or a new one from pool, at the
   stack manager's discretion).
4. The stack depth does not change. The back button goes to the screen BEFORE
   the replaced one.

Use for: redirects after form submission, auth-flow completion (replace login
screen with main app), tab switches within a navigation context.

### `External`

The platform hands the URL to the OS:
- **iOS:** `UIApplication.shared.open(url)` — opens in Safari or the default
  browser.
- **Android:** `Intent(Intent.ACTION_VIEW, uri)` — opens in Chrome or the
  default browser.
- **Desktop:** `open::that(url)` — opens in the system browser.

The in-app WebView is not involved. No platform services are available. No
auth tokens are attached.

Use for: external links (privacy policy, terms of service, third-party sites),
OAuth flows that require the system browser, deep links into other apps.

### `Root`

Clear the entire navigation stack and set this route as the new root. The
stack manager:

1. Captures screenshots of all current screens (for debugging, optional).
2. Destroys or pools all existing WebViews in the stack.
3. Creates a fresh WebView for the new root screen.
4. Loads the route.
5. The back button is disabled — there's nothing to go back to.

Use for: login → main app transition, deep link that resets the app state,
"sign out" that returns to the landing page.

---

## How `ViewKind` drives rendering surface selection

### `WebView` (default)

The platform creates or reuses a WebView for this route. This is the default for
all `RouteSource` variants — local WASM, IPC shell responses, and remote
content all flow through a WebView unless explicitly marked as `Native`.

**What the platform does:**
1. Acquires a WebView from the pool (or creates one) — see
   [decision 21](10-multi-webview-stack.md).
2. Resolves the `RouteSource` to get content (Step 5 of the execution contract).
3. Opens the appropriate transport lane and delivers bytes to the WebView.
4. **Steps back.** The platform does NOT interpret the bytes.
   `foundation-wasm-ui.js` (the bootstrap script injected into every WebView)
   receives the response, checks the `Content-Type`, and routes to the correct
   rendering pipeline:
   - `application/wasm` → instantiate WASM module, call `main()`
   - `text/html` → load as full document or morph fragment (depending on context)
   - `application/primal-columnar` → decode DomOps, apply to DOM
   - `application/vnd.apache.arrow.stream` → route through signal graph
   - etc.

The platform doesn't need a `RenderMode` enum to know which pipeline
`foundation_wasm_ui` will invoke. The bootstrap script handles that. If
`foundation_wasm_ui` adds a new rendering pipeline (Canvas, WebGPU, etc.), the
platform doesn't change.

### `Native`

The platform instantiates a platform-native OS view for this route. No WebView
is created. The native view component is registered with the platform's native
view registry and identified by a view ID in the `RouteDecision`.

### How sources communicate with native views

All sources talk to native views through the **session backbone**, not through
ad-hoc channels. The pattern is the same for every source:

```
WebviewApp code → session backbone → native view (request)
                                     native view → session backbone → WebviewApp code (response)

IpcShell code  → session backbone → native view (request)
                                     native view → session backbone → IpcShell code (response)

RemoteServer   → session backbone → native view (request)
                                     native view → session backbone → RemoteServer (response)
```

**Request/response through the capability bridge:**

A capability request is the universal primitive. Whether the source is code in a
WebView, WASM running in wasmtime inside the shell, or native Rust — it sends a
`CapabilityRequest` through the session backbone just like any other capability
call ([decision 18](07-native-capability-contract.md)):

```rust
// Example: WebviewApp code asks a native view to show a confirmation dialog.
// The source sends a capability request:
CapabilityRequest {
    id: request_id,
    page_identity: current_page,
    capability: "native_dialog",
    action: "confirm",
    payload: json!({ "title": "Delete item?", "ok": "Delete", "cancel": "Keep" }),
}

// The session backbone routes it to the registered handler for "native_dialog".
// The native view shows the OS dialog, captures the result, and responds:
CapabilityResponse {
    id: request_id,
    page_identity: current_page,
    status: Ok,
    payload: json!({ "confirmed": true }),
}
// The response is delivered back to the source, scoped to the requesting page.
```

**What makes this work regardless of source location:**

| Source | How it sends a request | How it receives the response |
|---|---|---|
| `WebviewApp` (WASM in WebView) | JS bridge → Tauri `invoke()` → session backbone | Session backbone → JS bridge → delivered to WASM as a signal/event |
| `IpcShell` (wasmtime in native shell) | Direct call into the session backbone (same process, no serialization needed for the request envelope) | Session backbone → callback registered with the request |
| `IpcShell` (native Rust .a/.so) | Direct function call into `PlatformSession::request()` | Same — callback or channel |
| `RemoteServer` | Not applicable — remote servers don't send capability requests TO native views. They send content; the content may trigger a local capability request. | N/A |

The key insight: **the session backbone is the universal message bus.** It
doesn't matter where the requester lives — the session routes the request to
the registered handler and delivers the response back. The native view doesn't
know or care whether the request came from a WebView, wasmtime, or native code.

**What the platform does:**
1. Looks up the registered native view component by view ID.
2. Instantiates the native view through the platform's native bridge:
   - **iOS:** Creates a `UIViewController` (or `UIHostingController` for SwiftUI)
     via the Tauri native bridge.
   - **Android:** Creates a `Fragment` or `ComposeView` via JNI.
   - **Desktop:** Creates a native window or embedded widget.
3. Resolves the `RouteSource` to get data for the native view.
4. Passes the `NavigationIntent` and initial data payload to the native view.
5. **Steps back.** The native view owns its rendering surface. It communicates
   with the platform through the capability bridge — sending capability requests
   and receiving responses through the session backbone, exactly like a WebView
   screen.

**Native views in the navigation stack:**

The WebView stack manager ([decision 21](10-multi-webview-stack.md)) is extended
to handle native views as peers to WebView slots. A `WebViewSlot` becomes a
`ScreenSlot` that holds either a WebView or a native view handle:

```rust
enum ScreenSlot {
    WebView {
        webview: Webview<R>,
        screenshot: Option<Vec<u8>>,
        state: SlotState,
    },
    Native {
        view_handle: NativeViewHandle,
        state: SlotState,
    },
}
```

Native views participate in the same screenshot-swap stack simulation — they
can be pushed, popped, replaced, and modally presented alongside WebView
screens. The same `Presentation` enum applies to both.

**Learning from Hotwire Native:**

Hotwire Native's `Navigator` manages two stacks: `session` (main) and
`modalSession` (modal). Each can contain a mix of `VisitableViewController`s
(WebView screens) and native `UIViewController`s. Our model generalizes this:
the stack manager holds `ScreenSlot`s, each of which is either a WebView or a
native view. The `Presentation` enum determines the transition; the `ViewKind`
determines the slot type.

Hotwire Native's `PathConfiguration` maps URL patterns to presentation rules
AND native view controller registration. Our route handler chain is the same
concept — but code, not JSON. The server CAN still influence routing decisions
(a route handler can fetch server-provided routing tables), but the compiled
handler is the final authority.

---

## How `source` drives protocol and transport selection

The `RouteSource` constrains which transport lanes and protocols are available:

| `RouteSource` | Available transports | Typical protocol | Latency |
|---|---|---|---|
| `WebviewApp` | No transport needed — app code runs in-process in the WebView. The session signals the route change (postMessage); the in-WebView code renders directly. | Whatever the in-WebView code produces (columnar v1, Arrow binary, JSON). | Zero — no process boundary. |
| `IpcShell` | Tauri command IPC (control lane). Native shell IPC (data lane — shared-memory Arrow). Custom protocol binary response. | Arrow binary — raw RecordBatch bytes (zero-copy in same process). Columnar v1 for DomOps. | Microseconds — pointer write, not memory copy (same process). |
| `RemoteServer` | HTTP fetch (request/response). SSE (unidirectional stream). WebSocket (bidirectional stream). Tauri custom protocol (proxies to remote). | Any — server decides. Columnar v1, HTML, JSON, Arrow binary. | Network round-trip time. |

Cache is not a separate source — `CachePolicy` controls whether and when the
cache layer intercepts requests for any of the three sources above. When a
cached response is served, it uses the same transport lane as the original
source (custom protocol for local, HTTP for remote). The transport is selected
automatically based on `RouteSource` and the available lanes. The user doesn't
manually pick a transport — the `RouteSource` implies it:

- `WebviewApp` → in-memory channel (no transport needed).
- `IpcShell` → Tauri command IPC + shared memory for data.
- `RemoteServer` → the session opens the best available transport for the URL
  scheme: `ewe://` = custom protocol, `ewe+ws://` = WebSocket, `ewe+http://`
  = HTTP fetch, `https://` = direct HTTP fetch.

Full transport lane definitions, Tauri integration points, and protocol
selection priority are in [decision 03](03-session-backbone-transport.md).

---

## How `profile` gates platform service access

Each `Profile` defines which platform services the content on this route can
access. The session backbone enforces this at runtime — every service call
checks the active profile.

At a high level:

| Profile | foundation_db | foundation_auth | foundation_nativeapis | foundation_http | Tauri commands |
|---|---|---|---|---|---|
| `App` | Full read/write | Full access | All capabilities | Full (all origins) | All commands |
| `TrustedRemote` | Read-only scoped queries | Auth-protected resources only (token attached by shell) | Route-allowlisted only | Allowed origins only | Allowlisted subset |
| `UntrustedRemote` | None | None | None | Same-origin fetch only | None |
| `Auth` | None (auth session only) | Login/logout/refresh only | Biometric only (if configured) | Auth provider origin only | None |
| `Devtools` | Full (dev only) | Full (dev only) | Full (dev only) | Full (dev only) | All (dev only) |

The full per-service breakdown (CSP directives, storage isolation, cookie jar
separation, navigation restrictions) is in [decision 14](06-webview-profiles.md).

The profile is assigned in `RouteDecision.profile`. The session backbone
enforces it:

```rust
// Every platform service checks the profile before executing:
impl DatabaseHandle {
    pub fn query(&self, session: &PlatformSession, sql: &str) -> Result<Rows> {
        session.check_profile_access(Service::Database, Access::Read)?;
        self.inner.query(sql)
    }
}

impl AuthManager {
    pub fn get_token(&self, session: &PlatformSession) -> Result<AuthToken> {
        session.check_profile_access(Service::Auth, Access::Read)?;
        self.inner.get_scoped_token(&session.route_identity)
    }
}
```

Cross-profile isolation is enforced by the session backbone:
- Separate cookie jars per profile (`auth` is isolated from `app`).
- Separate WebView storage (LocalStorage/SessionStorage) per profile.
- Capability requests carry profile identity — `untrustedRemote` requests are
  denied even if the capability is globally enabled.
- Content from different profiles renders in separate WebView contexts (or
  origin-isolated frames within the same WebView).

---

## How `cache_policy` controls offline behavior

The cache policy in the `RouteDecision` determines whether and how content is
cached, and what happens when the network is unavailable. Full details in
[decision 05](05-offline-and-sync.md).

| Policy | Cache behavior | Offline behavior | Best for |
|---|---|---|---|
| `CacheFirst` | Serve from cache. Fetch on miss. | Works offline (cached content). | Static content, offline-first apps. |
| `NetworkFirst` | Try network. Fall back to cache. | Serves stale cache. | Dynamic content that should be fresh. |
| `OnlineOnly` | Never cache. Always network. | Fails with offline error. | Real-time data, auth-gated content. |
| `LocalOnly` | Always cache. Never network. | Always works (no network needed). | Bundled content. |
| `StaleWhileRevalidate` | Serve cache, refresh in background. | Serves stale cache. | Content that should feel instant. |

The cache policy is assigned per-route in the `RouteDecision`. Different
routes can have different policies. The same route can have different policies
depending on context (online vs offline, authed vs unauthed).

---

## How `capabilities` are enforced per-route

The `RouteDecision.capabilities` list is a per-route allowlist. It adds a
defense-in-depth layer on top of the profile's broader service gates:

1. **Profile-level gate:** Is the `profile` allowed to use `foundation_nativeapis` at all?
   `UntrustedRemote` → no. Stop here.

2. **Capability registration gate:** Is the capability registered in the
   platform's capability registry? If not, the platform doesn't know how to
   handle it. Stop here.

3. **Per-route allowlist gate:** Is this specific capability in the route's
   `capabilities` list? If not, the capability is blocked on THIS route even
   though it's globally registered and the profile allows native APIs. Stop
   here.

4. **OS permission gate:** Has the user granted the OS-level permission
   (camera, microphone, location)? The platform checks before invoking the
   capability handler.

5. **Stale-page guard:** Is the requesting page still the active page? If the
   user navigated away, the response is discarded.

Example:

```rust
// Camera capability is globally registered:
session.register_capability::<CameraCapability>();

// Route A: camera IS allowed
session.route("/video/call/*", RouteDecision::webview_app()
    .with_profile(Profile::App)
    .with_allowed_capabilities(&[CapabilityId::camera]));

// Route B: camera is NOT allowed (not in the list)
session.route("/chat/*", RouteDecision::webview_app()
    .with_profile(Profile::App));

// A capability request from /chat/messages for camera is DENIED at gate 3,
// even though the profile allows all capabilities and camera is globally
// registered. The route's allowlist doesn't include it.
```

Full capability contract details (trait, proc macro, native bridges,
permission model) are in [decision 18](07-native-capability-contract.md).

---

## Where it lives: crate boundaries

Per [decision 01](01-platform-and-crates.md), the types are split across two crates:

### In `foundation_ui_traits` (dependency-free, shared by all crates)

Pure data types — no Tauri dependency, no WASM dependency, no I/O:

- `RouteDecision`, `RouteSource`, `NavigationIntent`, `IntentSource`
- `Presentation`, `ViewKind`, `ProtocolHint`
- `CachePolicy`, `Profile`, `CapabilityId`
- `SessionId`, `PageIdentity`

These types are pure enums and structs. Any crate can import them without
pulling in heavy dependencies.

### In `foundation_platform` (Tauri-dependent, integration crate)

Implementation types — Tauri-specific, platform-specific:

- `RouteHandler` trait — references `PlatformSession` and `NavigationIntent`
- `PlatformSession` struct — wraps `AppHandle<R>`, holds handler chain,
  capability registry, native view registry, cache handle, transport lane
  references
- `PatternRouter` — impl of `RouteHandler` (convenience)
- `FnRouteHandler` — impl of `RouteHandler` for closures (convenience)

### Why this split

`foundation_wasm_ui` (WASM side) imports the types from `foundation_ui_traits`
to track page identity, apply protocol selection, and respect profile-aware
rendering hints. No Tauri dependency.

`foundation_platform` (native side) imports the same types and wraps them in
Tauri-specific execution. Heavy Tauri dependency.

The `RouteHandler` trait lives in `foundation_platform` because its `resolve`
method takes `&PlatformSession`, which wraps Tauri's `AppHandle`. The data
types it returns live in `foundation_ui_traits` so both sides can use them.

---

## Why code, not config

Route policy needs arbitrary logic: database queries, auth state checks,
session context inspection, conditional branching. A config file constrains
decisions to what can be expressed in a static format. The trait approach
lets users run Rust.

Server-provided policy hints (stored in the session's path configuration,
like Hotwire Native's `PathConfiguration`) can still influence handler
decisions — a `RouteHandler` impl can fetch server-provided routing tables
and use them. But the handler is the final authority: local compiled policy
overrides server suggestions.

If a user WANTS a config-file approach, they can build one on top of the
trait — parse a JSON/YAML manifest, feed it into a `PatternRouter`. The
platform doesn't dictate the source of routing data.

---

## Why three APIs

Users have different needs. A simple app with local vs remote routes just
wants pattern matching (C). An app with per-route auth checks or dynamic
behavior wants a closure (B). A complex app with shared decision logic across
screens wants a trait impl (A). All three are the same type system
underneath — they register as `Box<dyn RouteHandler>` in the session's handler
chain. The execution contract is identical regardless of which API the user
chose.
