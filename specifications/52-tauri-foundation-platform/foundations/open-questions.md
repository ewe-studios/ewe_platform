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

**Resolved.** See `decisions/18-native-capability-contract.md`.
`Capability` trait + `#[platform_capability]` proc macro + registry builder.
Same three-API pattern as route handlers. Native bridges (Swift/Kotlin) for
where Tauri falls short — same trait, platform-specific impl. Wire format
in `foundation_ui_traits`: `CapabilityRequest`/`CapabilityResponse` with
`PageIdentity` scoping. All capability responses re-enter the rendering loop
through the session backbone.

### 8. Remote UI security

**Resolved.** See `decisions/19-remote-ui-security-red-team.md`.
Six defense layers (transport, profile sandboxing, capability gating, data
isolation, navigation security, runtime integrity). 8 red-team attacks
enumerated with mitigations. Server-instructed configuration model: server
can request/reduce privileges, never grant new ones. Local compiled policy
is the final authority. The platform is an explicit capability mediation layer
on top of Tauri's build-time security, not a Tauri wrapper.

### 9. Testing strategy

**Resolved.** See `decisions/20-testing-strategy.md`.
`#[platform_test]` proc macro with 5 targets: in-process Tauri WebView
(fast, unit-test-like), Chromium via CDP (full rendering), Android emulator
via ADB, iOS simulator via simctl, Firefox/BiDi (future). Follows
`foundation_browser`'s Harness pattern: deterministic setup/teardown,
panic-safe, screenshot on failure. `PlatformTestSession` API for route
inspection, capability mocking, cache inspection, event capture, lifecycle
simulation. CI: in-process on every commit, browser on PR, mobile on merge.

### 10. Communication lanes: all v1, shell owns it

**Resolved.** See `decisions/04-transport-lanes.md` and API surface docs
(decisions 09-13). All 7 transport lanes are v1. Every app ships with the
shell. The shell owns communication regardless of deployment model.

## Deferred items: resolved

### UniFFI / direct native wrapper lane

**Resolved.** See `decisions/07-native-integration-model.md` and
`decisions/18-native-capability-contract.md`. Native bridges are supported
as a capability implementation option when Tauri plugins are insufficient.
UniFFI is the recommended path for iOS native bindings; JNI for Android.
Not deferred — it's part of the native integration model. The platform's
Rust API surface is the same regardless of whether the backend is a
UniFFI-wrapped library, WASM in the shell, or a Tauri plugin.

### Multi-WebView native-stack simulation

**Resolved.** See `decisions/21-multi-webview-stack.md`. This is v1. The
session backbone is designed for multiple WebView contexts from the start.
Single-WebView is the default. The stack manager provides: screenshot-swap
for instant perceived responsiveness, WebView pool for reuse, background
preloading, stale content detection, and native back-gesture integration.
Inspired by Basecamp's shared-WebView + screenshot pattern but extended
for Tauri's multi-WebView capabilities.

### Mobile background sync services

**Resolved.** See `decisions/22-background-sync.md`. Three-tier design: Tier 1
(foreground sync, v1, trivial), Tier 2 (background-aware: periodic fetch,
push-triggered refresh, deferred downloads, v1 with platform caveats),
Tier 3 (full background services, v2, Android foreground service only).
Offline mutation queue with order-preserving idempotent replay. Same
`#[platform_worker]` code runs across all tiers with different execution
constraints.
