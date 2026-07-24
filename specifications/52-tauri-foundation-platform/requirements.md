# Specification 52: Tauri Foundation Platform

## Overview
This specification defines the creation of a new `foundation_platform` crate to manage cross-platform (desktop, mobile, web) capabilities using Tauri. It will integrate with the existing WASM UI foundation established in `specifications/completed/39-foundation-wasm-ui`.

## Links
- Basecamp Hotwire Native: /home/darkvoid/Boxxed/@formulas/src.UIFrameworks/src.basecamp/Basecamp Apps/
- Tauri: /home/darkvoid/Boxxed/@formulas/src.rust/src.Tauri/src.tauri/
- FoundationWasmUI: /home/darkvoid/Boxxed/@dev/ewe_platform/specifications/completed/39-foundation-wasm-ui 

## Goals
- Create `foundation_platform` crate.
- Integrate Tauri for cross-platform support.
- Leverage learnings from Basecamp's Hotwire Native approach for server-rendered app integration.
- Ensure seamless integration with WASM UI.

## North Star (2026-07-24)

Platform presentation modules should truly work: **modal creates a native bottom sheet**,
**dialog creates native AlertDialog** with embedded WebView support, and all
presentation modes (Push, Replace, Morph, Root, External) work correctly on Android.

`foundation_platform_native` owns the full stack:
- **Native implementation** — Kotlin/Swift helpers registered via Tauri plugin bridge
- **IPC handler** — `PlatformIpc` impls that drive `WindowManager` + native dialogs
- **WASM wrappers** — typed structs imported by WASM apps, dispatched through
  the IPC FFI bridge
- **Trigger registry** — host→WASM events for dialog results / modal dismiss

Each crate ships **docs/** with `getting_started/`, `deep-dives/`,
`adding_native_capabilities/`, `connecting_to_wasm/` linked from README.md.

Validation: `platform_android` exercises every capability headfully on the emulator.
No capability is "done" without end-to-end proof — WASM → IPC → native → screenshot.

## Status
**Phase: Implementation.** Architecture resolved across 14 decisions. F42 (native
modules + modal/dialog capabilities) is the active workstream. Foundation crates
(`foundation_platform`, `foundation_platform_native`, `foundation_wasm`,
`foundation_wasm_ui`) compile and ship on all targets.

## Decisions (all resolved, 2026-07-04, updated 2026-07-17)
1. **[Platform architecture and crate boundaries](decisions/01-platform-and-crates.md)** — `foundation_platform` owns Tauri integration; shared types in `foundation_ui_traits`.
2. **[Route policy model](decisions/02-route-policy-model.md)** — `RouteHandler` trait + `NavigationIntent`/`RouteDecision` structs; three API surfaces.
3. **[Session backbone and transport lanes](decisions/03-session-backbone-transport.md)** — Platform session spans WASM UI + native side; `ewe://` custom protocol; 7 transport lanes.
4. **[Deployment surfaces](decisions/04-deployment-surfaces.md)** — 5 deployment modes + entrypoint annotations overview.
5. **[Offline model and cache tiers](decisions/05-offline-and-sync.md)** — Two-tier offline: local WASM execution + rendered page caching. Per-route cache policies.
6. **[WebView profiles](decisions/06-webview-profiles.md)** — Trust boundaries gating platform service access per route.
7. **[Native capability contract](decisions/07-native-capability-contract.md)** — Typed, permissioned, route-scoped capability registry.
8. **[Security model](decisions/08-security-model.md)** — Capability mediation layer + red team approach.
9. **[Testing strategy](decisions/09-testing-strategy.md)** — Macro-driven test harness; browser + device + Docker-based cross-platform coverage.
10. **[Multi-WebView stack](decisions/10-multi-webview-stack.md)** — Screenshot-swap + background preload for native-stack simulation.
11. **[Background workers](decisions/11-background-workers.md)** — `#[platform_worker]` and `#[platform_service]` — foreground/background execution, OS constraints, in-process services.
12. **[Mutation queue and conflict resolution](decisions/12-mutation-queue-and-conflict.md)** — Offline mutation queue, idempotent replay, version vectors, conflict strategies.
13. **[`#[wasm_app]` entrypoint and `foundation_wasmtime`](decisions/13-wasm-app-entrypoint.md)** — build.rs pipeline, code gen, generated wrappers, `WasmtimeBuilder`, `PackageDirectorate`, session registry, auto-routing, project template.
14. **[Docker-based test environments](decisions/14-docker-test-environments.md)** — dockurr images (linux/android/windows) with VNC, `TestEnvironmentBuilder` API, `#[platform_test(docker)]` macro variant, CI pipeline with image caching.

## Foundations
- [`basecamp-hotwire-native.md`](foundations/basecamp-hotwire-native.md) — What we learned from Basecamp's Hotwire Native sources.
- [`foundation-wasm-ui.md`](foundations/foundation-wasm-ui.md) — Existing WASM UI capability model and boundaries.
- [`tauri-integration.md`](foundations/tauri-integration.md) — Tauri as host/platform layer.
- [`platform-synthesis.md`](foundations/platform-synthesis.md) — Synthesis: what `foundation_platform` should be.

## Plan
1. ~~Outline initial decisions.~~ ✅ 13 resolved.
2. ~~Create predocs/00-plan.md.~~ ✅
3. ~~Explore Basecamp Hotwire Native sources.~~ ✅
4. ~~Explore Tauri sources.~~ ✅
5. ~~Write foundation documents.~~ ✅ 4 documents.
6. ~~Create `foundation_platform` crate skeleton.~~ ✅
7. ~~Resolve all 12 gaps.~~ ✅ [gaps.md](gaps.md)
8. ~~Generate feature tickets.~~ ✅ 13 features (see below)
9. **Implement features** (next — in dependency order)

## Features

| ID | Feature | Priority | Depends on |
|---|---|---|---|
| F00 | Crate scaffold + shared types | Critical | — |
| F01 | Session backbone | Critical | F00 |
| F02 | Route handler trait + pattern router | Critical | F01 |
| F03 | `ewe://` custom protocol + transport lanes | Critical | F01 |
| F04 | WebView profiles + access gates | High | F00 |
| F05 | Capability registry | High | F01, F04 |
| F06 | Single-WebView stack manager (v1) | High | F01, F02 |
| F07 | Cache tiers + per-route policies | High | F01 |
| F08 | LWW mutation queue (MVP) | Medium | F01 |
| F09 | Walking skeleton (end-to-end) | Critical | F01–F08 |
| F10 | Testing harness (`#[platform_test]`) | High | F01 |
| F11 | Entrypoint annotations + build pipeline | Medium | F00, F01 |
| F12 | MVP integration + demo app | Critical | F09–F11 |
| F13 | Cross-platform builds (Desktop, Android, iOS) | Critical | F12 |
| F14 | Android example app (platform_android) | Critical | F13 |
| F15 | iOS example app (platform_ios) | Critical | F13 |
| F16 | App crate structure | High | F00, F11 |
| F17 | App build pipeline | High | F16 |
| F18 | Backend transport | High | F02, F03 |
| F19 | WASM annotation target | High | F11, F16 |
| F20 | Test extraction and public API | Medium | F10 |
| F21 | Multi-app distribution and WebView | Critical | F14, F16, F17 |
| F22 | MobileDirectory: disk-backed asset serving | Critical | F21 |
| F23 | WASM-Native Capabilities | Critical | F05, F19 |
| F24 | ScriptInjector: auto-inject runtimes | Critical | F03, F19, F21 |
| F25 | IPC Registry: central IPC mechanism | Critical | F02, F23, F24 |
| F26 | Streaming Channels: Tauri Channels | Medium | F25 |
| F27 | WASM Runtime IPC & Capability Triggers (host→wasm) | Critical | F23, F25 |
| F28 | WASM ConcurrentQueue Stream Registry + JS Stream Objects | High | F25, F26, F27 |
| F29 | Platform completeness: zero stubs, full-stack platform | Critical | F06, F14, F18, F21 |
| F30 | Docker-based test infrastructure | High | F29, F13 |
| F31 | Platform UI components (remote_page, floating_nav) | High | F29, F24 |

| F32 | Docker agentic control: mouse, keyboard, display | Critical | F30, F14, F15 |
| F33 | Wasmtime shell: `#[wasm_app]` + `foundation_wasmtime` crate | High | F19 |
| F34 | Background workers: `#[platform_worker]` + typed channels | High | F01, F25 |
| F35 | Multi-WebView desktop: Tauri window lifecycle + screenshot-swap | High | F29, F06 |

**Post-MVP features** (deferred from decisions):
- Native view support (SwiftUI/Jetpack Compose)
- Full conflict resolution (decision 12)
