# Foundation Open Questions

## Purpose

This document tracks architectural questions for `foundation_platform`.
Resolved answers with full rationale live in the `decisions/` directory.
The foundation documents (`platform-synthesis.md`, `foundation-wasm-ui.md`)
provide synthesis and capability context, not decision rationale.

Current decision docs:
- `decisions/01-platform-architecture.md` — create foundation_platform, Tauri host
- `decisions/02-route-policy-model.md` — RouteHandler trait + three composable APIs
- `decisions/03-session-backbone.md` — central coordinator spanning both crates
- `decisions/04-transport-lanes.md` — protocols vs transports, all lanes v1
- `decisions/05-offline-model.md` — two-tier: local WASM + rendered page caching
- `decisions/06-state-and-communication-model.md` — server owns state, user chooses comms
- `decisions/07-native-integration-model.md` — hybrid webview+native, native shell+WASM
- `decisions/08-arrow-and-data-model.md` — Arrow for data, not UI; zero-copy directions

## Status: answers given, design work needed

These questions have direction from discussion but need design documents
before they become implementation decisions.

### 1. Application mode API surfaces

**Direction:** Regardless of whether the application logic is WASM-driven or
compiled as a native static library, the platform always provides a Rust shell.
The shell is universal. Each API surface gets its own `main()` entrypoint —
just like `foundation_wasm_ui` uses deployment target macros to define where
code runs. The shell supplies whatever that function needs to operate in that
surface.

**What's needed:** A decision doc for each API surface, exploring in depth:
- What does the shell provide for this entrypoint? (transports, session handle,
  capabilities, cache, IPC channel, protocol encoders)
- How does the user's `main()` wire up? What does the platform invoke and when?
- How does this surface work independently?
- How does it compose with other surfaces (e.g., local WASM + remote fallback)?
- What transports carry the protocol payloads?
- What does caching/offline look like?

**API surfaces to design (each gets its own decision doc):**
1. **Bundled WASM in WebView** — WASM compiled into the app, runs in WebView;
   talks to native shell over IPC. The "offline-first" surface.
2. **Native static library** — User compiles Rust as `.a`/`.so`; shell links
   it and calls directly over FFI. Zero-copy, same process. The "maximum
   performance" surface.
3. **Shell + WASM (native-side WASM runtime)** — User compiles to WASM; shell
   embeds a WASM runtime. Same-process zero-copy Arrow. The "best of both"
   surface.
4. **Remote server-driven** — User's WASM runs on a remote server; shell
   streams responses over SSE/WS/fetch. The "app-store bypass" surface.
5. **Cached/offline replay** — Shell serves from local SQLite cache when
   connectivity is absent; reconnects and merges when available.

**Goal:** Each surface designed to work well independently, and to compose with
others. The shell provides the common substrate. The entrypoints define what
the user wires up for each deployment target.

### 2. Route policy: what does the platform provide vs what does the user implement?

**Resolved.** See `decisions/02-route-policy-model.md`.

### 3. WebView profiles: options needed

**Resolved.** See `decisions/14-webview-profiles.md`.
Five profiles (`app`, `trustedRemote`, `untrustedRemote`, `auth`, `devtools`)
with per-service access gates for all platform services (`foundation_db`,
`foundation_auth`, `foundation_nativeapis`, `foundation_http`, etc.) and
bidirectional Tauri integration. Runtime mediation layer on top of Tauri's
build-time security model.

### 4. Custom protocol transport adapter

**Resolved.** See `decisions/15-custom-protocol-model.md`.
`ewe://` scheme with sub-schemes (`ewe+ipc://`, `ewe+ws://`, `ewe+http://`).
URI structure maps routes through session backbone. Protocol selection per
response (columnar, Arrow IPC, JSON, HTML). Binary streaming via chunked
transfer and WebSocket upgrade. Full Tauri `UriSchemeProtocol` integration.

### 5. Bootstrap, packaging, and entry points

**Resolved.** See `decisions/16-entrypoint-model.md`.
Unified mode taxonomy extends `#[wasm_bin]`/`#[wasm_worker]`/`#[wasm_service]`
with `#[platform_bin]`/`#[platform_worker]`/`#[platform_service]`. Web modes
map to platform behavior via deep Tauri integration (service worker → in-process
HTTP server, web worker → background thread). `PlatformBundleGenerator` discovers
all six modes and compiles for the target platform.

### 6. Crate boundaries

**Resolved.** See `decisions/17-crate-boundaries.md`.
Shared session types (`SessionId`, `PageIdentity`, `RouteDecision`,
`NavigationIntent`, `CapabilityRequest`/`Response`, `Profile`, `CachePolicy`,
etc.) live in `foundation_ui_traits` — the existing dependency-free shared
boundary layer. `foundation_wasm_ui` imports them for the web-side session.
`foundation_platform` ties everything together, depends on Tauri + all
foundation crates, and provides the native-side session impl.

### 7. Native capability contract: the Rust API surface

**Answer:** Capabilities are `foundation_wasm_ui` primitives, not Basecamp-style
HTML-injected bridge components. Rendering is owned by the runtime. Capabilities
are declared in Rust, registered with the platform, and invoked through the
session backbone.

**Open design questions:**
- What does a capability declaration look like in Rust? `#[platform_capability]`
  proc macro on a function? A `Capability` trait? A registry builder pattern?
- How are permissions declared and enforced?
- Wire format is clear (capability name, action, typed payload, route/session
  scope, stale-page guard) — the Rust API for declaring and registering them
  is what needs design.
- How do capability results re-enter `foundation_wasm_ui`? (Event, signal,
  DomOp, command response, resource update?)
- Testing: we have `foundation_testbed` as precedent, and Tauri's Android/web
  test coverage. iOS strategy needs definition.

### 8. Remote UI security

**Answer:** We define permissions and privileges explicitly. Local compiled
policy is the final authority. Server policy can request capabilities but
cannot grant new ones that local policy denies.

**Important architectural note:** Tauri's capability/permission model is
primarily build-time (configured in `tauri.conf.json`). If we want per-route,
per-session capability decisions modifiable at runtime (e.g., server-suggested
policy adjustments), we are building a capability mediation layer *on top of*
Tauri's model, not just wrapping it. This should be explicit in the design.

**What's needed:**
- How does the platform layer runtime capability decisions on top of Tauri's
  build-time model?
- What surface does the platform expose for defining capabilities and their
  permission boundaries?
- How are tokens stored and scoped? Can remote UI load third-party scripts?

### 9. Testing strategy

**Answer:** Follow the precedent: `foundation_testbed`, `foundation_wasm`,
and `foundation_wasm_ui` already make browser/WebView testing work via test
macros. We integrate deeply with Tauri's test infrastructure and extend the
same pattern into `foundation_testbed` or `foundation_platform` — making a
mobile app or web page testable as simply as a test function with a macro
on top, matching the existing browser test experience.

**What's needed:** Design the platform test harness once implementation
surface is clearer. This is not blocking architecture decisions.

### 10. Communication lanes: all v1, shell owns it

**Answer:** We build all communication lanes and align with Tauri. Every app
ships with the shell regardless of deployment model (shell + compiled app code
as static lib, shell + WASM, or shell that loads a remote app). The shell owns
communication — between WebView and native APIs, between the app and remote
servers, between IPC peers — via Tauri's primitives and whatever custom code
we write.

**Resolved.** No further design discussion needed. Implementation will follow
the lane taxonomy already defined in `platform-synthesis.md`.

## Deferred items: v1 or v2?

These were listed as deferred but challenged in discussion. Each needs a
decision on whether it's v1, v2, or explicitly out of scope.

### UniFFI / direct native wrapper lane

Originally deferred ("not a v1 requirement"). Challenged: "why not support it?"

UniFFI requires generating Swift/Kotlin bindings, maintaining UDL files, and
testing against two mobile platforms. It's real engineering, not just a feature
flag. It's valuable (zero-copy Arrow across Rust↔Native in same process), but
it's a separate concern from the core platform APIs.

**Options:**
- **v1:** We build UniFFI integration into the platform from the start.
- **Tier 2:** Supported, documented, but not blocking v1. The platform's Rust
  API surface works the same whether the backend is a UniFFI-wrapped library
  or WASM in the native shell.

**Needs decision.**

### Multi-WebView native-stack simulation

Originally deferred. Challenged: "doesn't Basecamp already do this? Does Tauri
make it impossible?"

Basecamp does it by moving a shared `WKWebView` between view controllers and
using screenshots for inactive screens. Tauri doesn't make it impossible, but
it doesn't provide it out of the box — Tauri's model is one WebView per window
(or per child webview). Multi-WebView stack simulation requires managing
WebView lifecycle, screenshot caching, and navigation state ourselves.

It's non-trivial but important for native-feeling navigation. For simple
single-screen apps it's unnecessary. For production apps, you want it.

**Options:**
- **v1:** We design the session backbone to support multiple WebView contexts
  from the start, even if v1 ships with single-WebView default.
- **v2:** Single WebView for v1, multi-WebView stack simulation when needed.

**Needs decision.**

### Mobile background sync services

Originally deferred ("platform-constrained, focus on foreground + active-app
work first"). Challenged: "does Tauri not make this possible?"

Tauri gives us Rust async tasks. Mobile OSes (iOS especially) aggressively
kill background work. Tauri v2 has some plugin support for platform-specific
background APIs (BGTaskScheduler on iOS, WorkManager on Android), but it's not
a first-class primitive. We can build it, but it's platform-specific work that
requires per-OS testing.

**Options:**
- **v1:** We design the sync/cache layer to be background-aware from the start.
- **v2:** Foreground sync + active-app work for v1. Background sync when
  needed, using Tauri's plugin surface.

**Needs decision.**

## Summary

| # | Question | Status |
|---|----------|--------|
| 1 | App mode boundaries | Design doc needed: mode-by-mode analysis to find common abstractions |
| 2 | Route policy API | Design doc needed: interception + dispatch trait surface |
| 3 | WebView profiles | Decision doc needed: options with trade-offs |
| 4 | Custom protocol adapter | Design doc needed: Tauri transport adapter surface |
| 5 | Bootstrap / packaging | Design needed: entry point annotations, Tauri packaging alignment |
| 6 | Crate boundaries | Decision needed: where shared session types live |
| 7 | Capability contract | Design needed: Rust API for declaring/registering capabilities |
| 8 | Remote UI security | Design needed: capability mediation layer on top of Tauri's model |
| 9 | Testing | Follow precedent; design once implementation surface is clearer |
| 10 | Communication lanes | **Resolved:** all v1, shell owns communication |
| 11a | UniFFI | **Needs decision:** v1, tier 2, or out of scope? |
| 11b | Multi-WebView stacks | **Needs decision:** design for it in v1, or defer to v2? |
| 11c | Background sync | **Needs decision:** background-aware design in v1, or defer to v2? |
