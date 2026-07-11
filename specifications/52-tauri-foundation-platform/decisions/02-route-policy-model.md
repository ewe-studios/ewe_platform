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
6. [How each `render_mode` works](#how-each-render_mode-works)
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
    /// Full URL being navigated to (e.g. "ewe://localhost/app/items/42?proto=arrow-ipc")
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
            Some(RouteDecision::local_wasm()
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
        Some(RouteDecision::local_wasm())
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
session.route("/app/*", RouteDecision::local_wasm());
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
session.route("/app/*", RouteDecision::local_wasm());              // C
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

    /// What format the content is rendered in — determines which
    /// rendering pipeline the platform invokes.
    render_mode: RenderMode,

    /// Preferred wire protocol for delivering the content.
    /// The backend can override this.
    protocol: ProtocolHint,

    /// Cache behavior for this route (offline strategy).
    cache_policy: CachePolicy,

    /// WebView trust profile for this route (gates platform services).
    profile: Profile,

    /// Native capabilities allowed on this route (per-route allowlisting).
    capabilities: Vec<CapabilityId>,
}
```

### `RouteSource` — where content comes from

```rust
enum RouteSource {
    /// Content is generated by WASM running locally in the WebView.
    /// The app WASM module handles this route.
    LocalWasm,

    /// Content is generated by a native shell process on the device.
    /// The native IPC lane delivers it.
    IpcShell,

    /// Content is fetched from a remote server over the network.
    /// The appropriate transport lane (HTTP, SSE, WebSocket) carries it.
    RemoteServer,

    /// Content is served from the local cache.
    /// The cache layer provides the stored rendered page.
**TODO**: This is rather odd, because content can be cached regardless of the first 3, so why make it a source, probably better to add a cache, and then its that a cache wraps existing sources.
    Cache,
}
```

Convenience constructors:

```rust
RouteDecision::local_wasm()    // RouteSource::LocalWasm
RouteDecision::ipc_shell()     // RouteSource::IpcShell
RouteDecision::remote_fetch()  // RouteSource::RemoteServer
```

A from_cache that indicates cache delivers the content first before going to source
```rust
RouteDecision::from_cache(RouteDecision::local_wasm/ipc_shell/remote_fetch, CachePolicy)    // RouteSource::Cache
```

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

The distinction between `presentation` and `render_mode`:

- **`presentation`** controls the **navigation stack** — where the screen lives
  in the stack hierarchy, what transition plays, how the back button behaves.
  It drives the WebView stack manager ([decision 21](10-multi-webview-stack.md)).

- **`render_mode`** controls the **content rendering pipeline** — what format
  the content is in, which rendering subsystem processes it, how it becomes
  pixels. It drives the UI runtime in `foundation_wasm_ui`.

These are orthogonal. You can `Morph`-replace a `WasmApp` screen or
`Push`-navigate to an `HtmlDocument` screen. The stack manager handles
presentation; the rendering lane handles render_mode.

### `RenderMode` — what format the content is in

```rust
enum RenderMode {
    /// The response is a full WASM application module.
    /// foundation_wasm_ui instantiates the WASM, runs its main(),
    /// and the WASM takes over rendering for this screen.
    WasmApp,

    /// The response is a complete HTML document.
    /// The WebView renders it as a new page (like a browser loading a URL).
    /// foundation_wasm_ui's JS runtime re-initializes on the new document.
    HtmlDocument,

    /// The response is a stream of DomOps (binary columnar or JSON).
    /// foundation_wasm_ui's runtime receives the stream, applies each
    /// DomOp to the current DOM, continuously updating the screen.
    /// This is the default for server-driven content — the server
    /// pushes DomOps patches; the runtime applies them without a full reload.
    DomOpsStream,

    /// The response is an HTML fragment.
    /// foundation_wasm_ui's runtime morphs the fragment into the existing
    /// DOM at a target element. Only the changed portion is replaced.
    /// Used for partial page updates, Turbo/Hotwire-style replacements.
    FragmentMorph,

    /// The response is structured data (Arrow IPC or JSON).
    /// foundation_wasm_ui's runtime projects it through a signal graph
    /// or template. The data drives the UI; the rendering is
    /// template-defined, not content-defined.
    DataProjection,
}
```

How each mode works concretely:

**`WasmApp`:**
1. The platform loads the user's WASM module into the WebView's WASM runtime.
2. `foundation-wasm-ui.js` instantiates the module, calls its exported `main()`
   with a `PlatformSession` handle.
3. The WASM owns the rendering surface for this screen — it uses the `html!`
   macro, signals, templates, and components from `foundation_wasm_ui`.
4. The platform's rendering lane is not involved after bootstrap — the WASM
   drives everything from inside the WebView.

**`HtmlDocument`:**
1. The backend (remote server, local shell, or cache) returns a full HTML
   document: `<html><head>...</head><body>...</body></html>`.
2. The platform delivers it through the transport lane as `text/html`.
3. The WebView loads it as a new document. `foundation-wasm-ui.js` is
   re-injected as an initialization script. The JS runtime re-bootstraps.
4. Used for server-rendered pages that are complete documents (landing pages,
   auth screens, static content).

**`DomOpsStream`:**
1. The backend streams binary columnar v1 DomOps (or JSON DomOps) over the
   transport lane.
2. `foundation_wasm_ui`'s runtime receives each batch, decodes the DomOps, and
   applies them to the current DOM.
3. The screen updates incrementally — no full reload, no document replacement.
4. This is the primary mode for server-driven UIs. The server pushes patches;
   the runtime applies them. Every interaction (click, form submit, etc.) is a
   DomOps exchange over the transport lane.

**`FragmentMorph`:**
1. The backend returns an HTML fragment: `<div id="content">...</div>`.
2. `foundation_wasm_ui`'s runtime identifies the target element in the current
   DOM (by `id` or CSS selector), computes a morph between the old fragment and
   the new one, and applies only the differences.
3. The surrounding page (navigation, sidebars, etc.) is untouched.
4. Used for Turbo/Hotwire-style partial updates — the server sends only the
   part of the page that changed, and only that part is updated.

**`DataProjection`:**
1. The backend returns structured data — Arrow IPC `RecordBatch` or JSON.
2. `foundation_wasm_ui`'s runtime receives the data, routes it through the
   signal graph or template system.
3. Reactive bindings update the DOM automatically — the data changes, the UI
   reflects it. No DomOps stream, no HTML fragment.
4. Used for data-heavy screens (dashboards, tables, analytics) where the
   structure is template-defined and only the data changes.

### `ProtocolHint` — preferred wire format

```rust
enum ProtocolHint {
    /// Use the backend's native format. Whatever the backend produces is
    /// delivered as-is. This is the default.
    Default,

    /// Prefer columnar v1 encoding (DomOps batches, wasm-loop, no-std).
    Columnar,

    /// Prefer Apache Arrow IPC (structured data, zero-copy).
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
4. Platform default — columnar v1 for DomOps, Arrow IPC for data payloads.

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
render as `HtmlDocument` with `UntrustedRemote` profile. The user can override
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
    RouteSource::LocalWasm => {
        // The WASM module is already loaded in the WebView.
        // The session signals the WASM to handle this route.
        // The WASM's main() or route handler generates the content.
        // Output: DomOps, HTML, or Arrow data (whatever the WASM produces).
    }
    RouteSource::IpcShell => {
        // Content comes from the native shell via IPC.
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
    RouteSource::Cache => {
        // Already handled in Step 3. If we reach here, it's a cache miss
        // with a policy that falls through — re-query the backend.
    }
}
```

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
- **`ewe+ipc://`:** Tauri command IPC — JSON/MessagePack serialized. Control
  lane for small typed request/response.
- **`ewe+ws://`:** WebSocket — bidirectional streaming for live DomOps,
  collaborative editing, real-time sync.
- **`ewe+http://`:** HTTP fetch — standard web semantics, proxies to dev
  server or remote.

### Step 8: Rendering

The rendering lane receives the encoded content and applies it using the
pipeline determined by `RouteDecision.render_mode`:

```
RenderMode::WasmApp       → WASM runtime instantiates module, calls main()
RenderMode::HtmlDocument  → WebView loads full document, JS runtime bootstraps
RenderMode::DomOpsStream  → Runtime applies DomOp batches incrementally to DOM
RenderMode::FragmentMorph → Runtime morphs HTML fragment into target element
RenderMode::DataProjection→ Runtime projects data through signals/templates
```

Full details in [How each `render_mode` works](#how-each-render_mode-works).

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
       render_mode: DomOpsStream,
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

10. Render: DomOpsStream.
    foundation_wasm_ui runtime receives the stream.
    Each DomOp batch is decoded and applied to the new screen's DOM.
    Screen renders incrementally.

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

## How each `render_mode` works

### `WasmApp`

The response is a WASM module. This is the mode for locally-executed
application code.

**What the backend sends:** A compiled `.wasm` binary (or a reference to an
already-loaded module).

**What the platform does:**
1. The WebView's WASM runtime (`foundation-wasm-ui.js`) instantiates the
   module.
2. Calls the module's exported `main()` function with a `PlatformSession`
   handle.
3. The WASM module takes over rendering for this screen. It uses
   `foundation_wasm_ui`'s APIs — `html!` macro, signals, templates,
   components, event handling.
4. All subsequent interactions on this screen are handled by the WASM module.
   No network round-trip needed (unless the WASM explicitly makes one).

**When to use:** Local-first apps where the application logic runs on-device.
Offline by default. The WASM is bundled with the app or hot-updated through
the shell's update mechanism.

**Crate dependency:** The WASM module depends on `foundation_wasm_ui`. The
platform crate is not in the WASM's dependency graph — it communicates with
the platform through the `PlatformSession` handle, which is a
`foundation_ui_traits` type.

### `HtmlDocument`

The response is a complete HTML document.

**What the backend sends:** A full HTML document with `<html>`, `<head>`,
and `<body>`. The response `Content-Type` is `text/html; charset=utf-8`.

**What the platform does:**
1. Delivers the HTML to the WebView as an HTTP response (via Tauri custom
   protocol or direct load).
2. The WebView renders it as a new page — it's a full document load.
3. `foundation-wasm-ui.js` is re-injected as an initialization script (per
   WebView profile's CSP).
4. The JS runtime bootstraps: attaches signal runtime, event listeners, custom
   protocol interceptors.
5. If the HTML includes `<script>` tags that load the user's WASM module, the
   WASM runtime instantiates it.

**When to use:** Server-rendered pages from a traditional web backend. Landing
pages, marketing content, auth screens (OAuth providers return HTML).
Interoperability with existing server-rendered web applications.

**How it relates to `foundation_wasm_ui`:** The HTML document may include
`foundation-wasm-ui.js` as a `<script>` tag. If present, the runtime
initializes and provides signal/template/component capabilities. If absent,
the page is a plain HTML document with no platform runtime — just standard web
content.

### `DomOpsStream`

The response is a stream of DOM operation batches. This is the primary mode
for server-driven UIs.

**What the backend sends:** A continuous (or batched) stream of DomOps —
instructions like "create element," "set attribute," "insert text," "remove
node." Encoded in columnar v1 binary, JSON, or Arrow IPC, depending on the
selected protocol.

**What the platform does:**
1. Opens a transport to the backend (HTTP streaming response, SSE, WebSocket,
   or IPC).
2. As each batch of DomOps arrives, the transport lane delivers it to the
   WebView as an `ArrayBuffer` (binary) or JSON object.
3. `foundation_wasm_ui`'s runtime decodes each batch and applies the DomOps
   to the current DOM:
   - **Binary columnar v1:** Decoded by the WASM runtime's no-std columnar
     decoder. TypedArray-friendly. Zero-copy where possible.
   - **JSON:** Decoded by the JS runtime. Slower but debuggable.
   - **Arrow IPC:** Decoded by Arrow JS or the WASM runtime's Arrow reader.
4. The DOM updates incrementally. The user sees the screen build up (or morph)
   as DomOps arrive.

**When to use:** Server-driven UIs where the backend owns rendering logic.
Real-time updates (collaborative editing, live dashboards, chat). Any screen
where the server pushes changes and the client applies them.

**How it relates to `foundation_wasm_ui`:** This is `foundation_wasm_ui`'s
core rendering path. The DOM applicator, columnar decoder, and morph engine
all live in `foundation_wasm_ui`. The platform provides the transport; the UI
runtime provides the rendering.

### `FragmentMorph`

The response is an HTML fragment that replaces a portion of the current DOM.

**What the backend sends:** An HTML fragment — not a full document. Typically
a `<div>` or `<turbo-frame>` with an `id` attribute. `Content-Type` is
`text/html; charset=utf-8`.

**What the platform does:**
1. Delivers the HTML fragment to the WebView.
2. `foundation_wasm_ui`'s runtime parses the fragment into a DOM subtree.
3. Identifies the target element in the current document (by matching `id` or
   a CSS selector from the fragment's root element).
4. Computes a morph between the existing element and the new fragment — finds
   the minimal set of DOM changes.
5. Applies only the differences: adds new nodes, removes old nodes, updates
   changed attributes and text. The rest of the page is untouched.
6. Optionally animates the transition (CSS transitions on changed elements).

**When to use:** Turbo/Hotwire-style partial updates. Server-rendered
templates where only a section changes (navigation stays static, content area
updates). Form submissions that return updated HTML for the form area.

**How it relates to `foundation_wasm_ui`:** The morph engine exists in
`foundation_wasm_ui` (it already handles DomOp-based morphing for
`DomOpsStream`). `FragmentMorph` uses the same engine but takes HTML fragments
as input instead of DomOps — the fragment is parsed, diffed, and morphed.

### `DataProjection`

The response is structured data. The rendering is template-driven.

**What the backend sends:** Structured data — Arrow IPC `RecordBatch` (for
tabular data), JSON objects (for nested data), or columnar v1 data batches.
The `Content-Type` reflects the format.

**What the platform does:**
1. Delivers the data to the WebView as an `ArrayBuffer` or JSON object.
2. `foundation_wasm_ui`'s runtime receives the data and routes it through the
   signal graph for this screen.
3. Reactive bindings update the DOM: a signal changes, the template re-renders
   the affected elements.
4. Subsequent data updates (new batches, server push) update the signals,
   which update the DOM. The template is defined once; data drives updates.

**When to use:** Data-heavy screens: dashboards, analytics tables, data grids,
charts. Any screen where the structure is known at build time and only the data
changes at runtime. Arrow IPC for zero-copy large datasets.

**How it relates to `foundation_wasm_ui`:** `foundation_signals` provides the
reactive signal graph. `foundation_wasm_ui` provides the template system and
the `html!` macro that binds signals to DOM elements. `foundation_arrow`
provides the Arrow IPC encoding/decoding on both sides.

---

## How `source` drives protocol and transport selection

The `RouteSource` constrains which transport lanes and protocols are available:

| `RouteSource` | Available transports | Typical protocol | Latency |
|---|---|---|---|
| `LocalWasm` | No transport needed — WASM generates content in-process in the WebView. Session handle passes data through in-memory channels. | Whatever the WASM produces (columnar v1, Arrow IPC, JSON). | Zero — no process boundary. |
| `IpcShell` | Tauri command IPC (control lane). Native shell IPC (data lane — shared-memory Arrow). Custom protocol binary response. | Arrow IPC (zero-copy in same process). Columnar v1 for DomOps. | Microseconds — pointer write, not memory copy (same process). |
| `RemoteServer` | HTTP fetch (request/response). SSE (unidirectional stream). WebSocket (bidirectional stream). Tauri custom protocol (proxies to remote). | Any — server decides. Columnar v1, HTML, JSON, Arrow IPC. | Network round-trip time. |
| `Cache` | Tauri custom protocol (serves from local SQLite). | Whatever was cached (stored as-encoded). | Local disk read. |

The transport is selected automatically based on `RouteSource` and the
available lanes. The user doesn't manually pick a transport — the `RouteSource`
implies it:

- `LocalWasm` → in-memory channel (no transport needed).
- `IpcShell` → Tauri command IPC + shared memory for data.
- `RemoteServer` → the session opens the best available transport for the URL
  scheme: `ewe://` = custom protocol, `ewe+ws://` = WebSocket, `ewe+http://`
  = HTTP fetch, `https://` = direct HTTP fetch.
- `Cache` → custom protocol handler serves from SQLite.

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
session.route("/video/call/*", RouteDecision::local_wasm()
    .with_profile(Profile::App)
    .with_allowed_capabilities(&[CapabilityId::camera]));

// Route B: camera is NOT allowed (not in the list)
session.route("/chat/*", RouteDecision::local_wasm()
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
- `Presentation`, `RenderMode`, `ProtocolHint`
- `CachePolicy`, `Profile`, `CapabilityId`
- `SessionId`, `PageIdentity`

These types are pure enums and structs. Any crate can import them without
pulling in heavy dependencies.

### In `foundation_platform` (Tauri-dependent, integration crate)

Implementation types — Tauri-specific, platform-specific:

- `RouteHandler` trait — references `PlatformSession` and `NavigationIntent`
- `PlatformSession` struct — wraps `AppHandle<R>`, holds handler chain,
  capability registry, cache handle, transport lane references
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
