# Foundation: Tauri Platform Integration Model

## Purpose of this foundation document

This document describes Tauri as the host/platform layer for
`foundation_platform`. It focuses on architectural boundaries: what Tauri owns,
what the WebView owns, how IPC and custom protocols fit, what changes on mobile,
and how these facts constrain a `foundation_wasm_ui` integration.

The main conclusion is:

> Tauri should be treated as the secure native host and platform-service runtime,
> while `foundation_wasm_ui` remains the DOM/UI runtime inside the WebView.

## Tauri's core architecture

Tauri builds native applications with a Rust backend and a system WebView
frontend. It does not bundle Chromium or Node.js. Instead, it uses the platform's
native WebView through WRY and event/window handling through TAO.

A simplified stack is:

```text
Application crate
        │
        ▼
tauri
        │
        ├── app/window/webview lifecycle
        ├── command IPC and event system
        ├── protocol and asset handling
        ├── plugin system
        ├── security/capability configuration
        └── build/bundle integration
        │
        ▼
tauri-runtime abstraction
        │
        ▼
tauri-runtime-wry
        │
        ├── WRY: WebView abstraction
        └── TAO: window/event-loop abstraction
        │
        ▼
Platform WebView / OS windowing
```

Platform WebViews:

| Platform | WebView |
|---|---|
| macOS | WKWebView |
| iOS | WKWebView |
| Windows | WebView2 |
| Linux | WebKitGTK |
| Android | Android System WebView |

The Android detail matters: Android does **not** use WebView2.

## Tauri is a host, not a UI framework

Tauri does not prescribe React, Svelte, vanilla JS, WASM, or server-rendered HTML.
It hosts any frontend that compiles to HTML/CSS/JS/WASM. For this project, that
frontend is primarily `foundation_wasm_ui`.

Therefore, `foundation_platform` should not define a new component system or DOM
runtime. It should provide:

- app lifecycle integration;
- WebView/window configuration;
- runtime asset packaging;
- native capability bridge;
- filesystem/cache/database services;
- secure network and custom protocol handling;
- build/dev/bundle conventions;
- platform-aware navigation policy.

## Runtime and WebView abstraction

Tauri separates app code from concrete WebView implementation through
`tauri-runtime` traits. The central ideas are:

- `Runtime` owns the event loop and creates windows/webviews;
- `RuntimeHandle` lets Rust code schedule work on the main thread and create
  windows/webviews;
- `WebviewDispatch` gives thread-safe access to WebView operations such as
  navigation, reload, script evaluation, URL lookup, focus, close, and event
  registration;
- `WebviewAttributes` describe WebView configuration.

This separation is valuable for `foundation_platform`: our abstractions should be
above the concrete WebView where possible, but should not hide platform-specific
constraints when they matter.

## WebView attributes and policy surface

Tauri WebView configuration includes concerns such as:

- initial URL;
- user agent;
- initialization scripts;
- data directory / data store;
- incognito mode;
- clipboard access;
- drag/drop;
- bounds and autoresize;
- proxy URL;
- devtools;
- JavaScript enable/disable;
- background throttling;
- link preview;
- autofill;
- custom browser args on supported platforms;
- HTTPS scheme behavior.

For `foundation_platform`, this suggests an explicit WebView profile/policy
object. Different app surfaces may need different policies:

- trusted bundled app shell;
- remote server-driven screen;
- authentication screen;
- sandboxed external content;
- offline cached page;
- internal diagnostics/devtools page.

## Desktop vs mobile constraints

### Desktop

Desktop Tauri generally has:

- a native process running a Rust event loop;
- one or more OS windows;
- one or more WebViews attached to windows;
- more flexible background process behavior;
- platform-specific WebView configuration;
- filesystem/database access through the Rust backend;
- easier devtools/debugging.

Desktop-specific WebView differences still matter:

- WebView2 profile/environment details on Windows;
- WebKitGTK and GTK integration concerns on Linux;
- WKWebView behavior and macOS app lifecycle on macOS.

### Mobile

Mobile Tauri changes the operational model:

- iOS uses WKWebView inside UIKit lifecycle constraints;
- Android uses Android System WebView with JNI/Kotlin/Activity integration;
- app background execution is constrained by the OS;
- network and storage permissions are platform-mediated;
- long-running background work may need platform-specific APIs;
- WebView lifecycle can be affected by memory pressure and app suspension;
- native project generation and mobile build chains are part of the platform
  story.

For `foundation_platform`, mobile support cannot be treated as “desktop but
smaller”. Navigation, cache durability, background sync, and permission prompts
need mobile-specific policy.

## Tauri communication surfaces

Tauri offers several communication surfaces. They should be used for different
jobs.

### Command IPC

Tauri commands expose Rust functions to the frontend. In typical use they carry
serialized structured values between JavaScript and Rust.

Good uses:

- small request/response operations;
- permission checks;
- app metadata;
- native dialog/file picker triggers;
- sync command initiation;
- small status results;
- capability calls with typed payloads.

Poor uses:

- large high-frequency data streams;
- raw bulk table transfer;
- DOM patch streaming if a better runtime protocol exists;
- anything that requires direct pointer sharing with WebView JavaScript.

For `foundation_platform`, command IPC should be the **control lane**, not the
universal data lane.

### Event system

Tauri's event system allows emitting events between Rust and frontend scopes.
This is useful for:

- lifecycle notifications;
- sync progress;
- online/offline state;
- native capability completion;
- background task updates;
- invalidation notifications;
- route/cache events.

Events should remain structured and scoped. They should include enough identity
to avoid stale updates, especially for route/page/session-specific operations.

### Initialization scripts and frontend API

Tauri can inject initialization scripts into the WebView. This is relevant for
bootstrapping:

- `foundation-wasm.js`;
- `foundation-wasm-ui.js`;
- platform bridge objects;
- route/session identity;
- feature flags;
- development diagnostics;
- capability registry metadata.

The platform should keep initialization deterministic and auditable. Injected
scripts are privileged and should not become an unstructured dumping ground.

### Script evaluation

Tauri can evaluate JavaScript in the WebView. This is useful for controlled
runtime operations, but it should not become the main protocol. Prefer explicit
runtime APIs and message channels over string-built JavaScript where possible.

### Custom protocols and asset serving

Tauri can serve application assets and custom protocol responses. This is one of
the most important surfaces for this spec.

A custom protocol can represent:

- bundled assets;
- generated runtime bootstrap;
- cached HTML;
- cached WASM/JS;
- local data projections;
- server-fetched content mediated by Rust;
- offline fallbacks;
- authenticated resources without exposing raw tokens to the WebView.

For a `foundation_wasm_ui` integration, custom protocols can let Rust own the
resource pipeline while the WebView consumes resources through normal browser
fetch/navigation mechanisms.

However, custom protocols should be designed carefully:

- define origin/security behavior;
- define cache headers and invalidation;
- avoid exposing arbitrary filesystem paths;
- normalize MIME types;
- separate trusted app assets from untrusted remote content;
- avoid using custom protocol as a hidden global privilege bypass.

## Security and capability model

Tauri's security model centers on explicit configuration, capability restrictions,
plugin permissions, CSP, and controlled IPC exposure. `foundation_platform` should
align with this instead of bypassing it.

Core security principles:

1. **Least privilege commands** — expose narrow commands, not a generic “run
   native action” endpoint.
2. **Capability registry** — native/platform capabilities should be named,
   typed, permissioned, and auditable.
3. **Origin-aware policy** — bundled app UI, remote UI, cached UI, and external
   content should not have identical privileges by default.
4. **Route-aware policy** — a capability may be allowed on one route and denied
   on another.
5. **No secret leakage** — tokens and credentials should stay in Rust/native
   storage where possible. The WebView should receive scoped results, not raw
   long-lived secrets.
6. **Message identity** — requests and responses should carry route/session/page
   identity to prevent stale or cross-screen delivery.
7. **Explicit binary lanes** — large/binary data should use deliberately designed
   transports, not accidental JSON IPC.

## Asset and runtime packaging

`foundation_platform` needs to package and serve several categories of assets:

- `foundation_wasm_ui` runtime JavaScript;
- application WASM modules;
- HTML boot documents;
- CSS/theme assets;
- server-rendered/cached HTML fragments;
- icons and static media;
- route/path configuration;
- development diagnostics tools;
- optional offline snapshots.

The platform should distinguish:

- build-time embedded assets;
- app-data cached assets;
- server-fetched assets;
- generated assets;
- developer-mode assets served from a dev server.

This distinction affects update policy, integrity checks, caching, and security.

## Data and transport lanes

A mature platform should separate lanes by purpose.

### Control lane: Tauri command IPC

For small, typed control messages.

```text
WebView -> invoke command -> Rust service -> response
```

### Event lane: Tauri events

For lifecycle, progress, invalidation, and completion notifications.

```text
Rust service -> event -> WebView runtime
```

### UI runtime lane: `foundation_wasm_ui` protocol

For DOM operations, morphing, event callbacks, and UI runtime messages.

```text
WASM/server/runtime -> DomOps/HTML/protocol payload -> JS runtime -> DOM
```

### Resource lane: custom protocol / fetch

For assets, cached pages, generated fragments, and data projections.

```text
WebView fetch/navigation -> custom protocol -> Rust resource service
```

### Storage/cache lane: Rust-owned services

For persistent local data and offline state.

```text
Rust services <-> SQLite / mmap / filesystem / foundation_db / foundation_arrow
```

### Future native plugin / FFI lane

For platform-specific high-performance native integration where Tauri commands
are insufficient.

This should be treated as optional/future until a concrete performance need and
safe memory model are proven.

## Arrow and binary payload implications

Apache Arrow is appropriate for high-throughput structured data. But the
transport still determines copy behavior.

Reasonable claims:

- Rust can store, process, and cache Arrow IPC data efficiently.
- WebView JavaScript can parse Arrow IPC bytes if delivered as an `ArrayBuffer`.
- `foundation_wasm_ui` already has distinct columnar and Arrow-family protocol
  paths.
- A custom protocol can deliver binary resources to the WebView in a browser-like
  way.
- Rust can keep large data local and expose slices/projections to the UI.

Claims to avoid until proven:

- native Rust memory pointers can be directly passed to WebView JavaScript;
- Tauri command IPC is zero-copy for Arrow buffers on all platforms;
- a mobile WebView can safely read arbitrary mmap pages owned by Rust;
- UniFFI/direct FFI is automatically available inside a Tauri WebView lane.

The more robust design is:

```text
Large data remains Rust-owned
        │
        ├── cached/stored as Arrow/SQLite/mmap where useful
        ├── transformed into renderable projections
        └── delivered to WebView through explicit resource/protocol/runtime lanes
```

## Navigation model implications

Tauri does not automatically provide Hotwire-style native navigation semantics.
A WebView can navigate internally like a browser, and Tauri can create windows or
navigate WebViews, but route-to-native-presentation policy must be designed by
`foundation_platform`.

Possible navigation abstractions:

- app route: logical location independent of URL;
- source: bundled, remote, cache, local protocol, external;
- presentation: same view, replace, modal, new window, external browser;
- render mode: WASM app, HTML document, HTML fragment, DomOps stream, island;
- offline policy: cache-first, network-first, online-only, local-only;
- privilege profile: trusted app, authenticated remote, untrusted external;
- lifecycle policy: preserve WebView, reload, cold boot, snapshot, dispose.

This is where Hotwire Native's path configuration lessons are most relevant.

## Mobile background work

Tauri/Rust can run background tasks while the app is active, but mobile operating
systems constrain background execution. `foundation_platform` should distinguish:

- active foreground async work;
- app-suspended work;
- OS-approved background fetch/sync;
- push-triggered refresh;
- platform-specific long-running services;
- local queue replay when app returns to foreground.

Offline mutation queues and sync engines should be designed to tolerate being
paused and resumed rather than assuming an always-running daemon.

## What belongs in `foundation_platform`

`foundation_platform` should own the integration surface around Tauri:

- Tauri app/bootstrap conventions;
- WebView profile/policy definitions;
- asset and custom protocol serving;
- route/navigation policy;
- command/event bridge wrappers;
- native capability registry;
- offline cache and sync orchestration interfaces;
- packaging hooks for `foundation_wasm_ui` runtimes;
- development server integration;
- security/capability defaults;
- platform-specific extension points.

It should not own:

- `html!` templates;
- signal graph semantics;
- DOM operation definitions;
- the JS DOM applicator;
- the function-call ABI;
- low-level Arrow encoders;
- app-specific business logic.

## Summary

Tauri provides the secure native host, WebView embedding, IPC/event channels,
custom protocol/resource serving, plugin model, and packaging story. It is the
right foundation for cross-platform shell behavior.

For this project, Tauri should host `foundation_wasm_ui`, not replace it. The
main design challenge for `foundation_platform` is choosing the right lane for
each responsibility: commands for control, events for notifications, custom
protocols for resources, `foundation_wasm_ui` protocols for rendering, and
Rust-owned storage/cache lanes for offline and large data.
