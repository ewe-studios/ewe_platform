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

## Status
**Phase: Implementation-ready.** Architecture resolved across 14 decisions. Four foundation documents capture the research. Skeleton crate exists (`backends/foundation_platform/`). Implementation begins.

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

**Post-MVP features** (deferred from decisions):
- Native view support (SwiftUI/Jetpack Compose)
- `#[wasm_app]` + `foundation_wasmtime` (decision 13)
- Full conflict resolution (decision 12)
- Surface 2 (native static lib) + Surface 3 (shell+WASM runtime)
- Background workers (foreground/background, OS constraints)
- Multi-WebView (desktop+unstable)
