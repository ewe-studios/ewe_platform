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
**Phase: Implementation-ready.** Architecture resolved across 13 decisions. Four foundation documents capture the research. Skeleton crate exists (`backends/foundation_platform/`). Implementation begins.

## Decisions (all resolved, 2026-07-04, updated 2026-07-17)
1. **[Platform architecture and crate boundaries](decisions/01-platform-and-crates.md)** — `foundation_platform` owns Tauri integration; shared types in `foundation_ui_traits`.
2. **[Route policy model](decisions/02-route-policy-model.md)** — `RouteHandler` trait + `NavigationIntent`/`RouteDecision` structs; three API surfaces.
3. **[Session backbone and transport lanes](decisions/03-session-backbone-transport.md)** — Platform session spans WASM UI + native side; `ewe://` custom protocol; 7 transport lanes.
4. **[Deployment surfaces](decisions/04-deployment-surfaces.md)** — 5 deployment modes + entrypoint annotations overview.
5. **[Offline model and cache tiers](decisions/05-offline-and-sync.md)** — Two-tier offline: local WASM execution + rendered page caching. Per-route cache policies.
6. **[WebView profiles](decisions/06-webview-profiles.md)** — Trust boundaries gating platform service access per route.
7. **[Native capability contract](decisions/07-native-capability-contract.md)** — Typed, permissioned, route-scoped capability registry.
8. **[Security model](decisions/08-security-model.md)** — Capability mediation layer + red team approach.
9. **[Testing strategy](decisions/09-testing-strategy.md)** — Macro-driven test harness; browser + device coverage.
10. **[Multi-WebView stack](decisions/10-multi-webview-stack.md)** — Screenshot-swap + background preload for native-stack simulation.
11. **[Background workers](decisions/11-background-workers.md)** — `#[platform_worker]` and `#[platform_service]` — foreground/background execution, OS constraints, in-process services.
12. **[Mutation queue and conflict resolution](decisions/12-mutation-queue-and-conflict.md)** — Offline mutation queue, idempotent replay, version vectors, conflict strategies.
13. **[`#[wasm_app]` entrypoint and `foundation_wasmtime`](decisions/13-wasm-app-entrypoint.md)** — build.rs pipeline, code gen, generated wrappers, `WasmtimeBuilder`, `PackageDirectorate`, session registry, auto-routing, project template.

## Foundations
- [`basecamp-hotwire-native.md`](foundations/basecamp-hotwire-native.md) — What we learned from Basecamp's Hotwire Native sources.
- [`foundation-wasm-ui.md`](foundations/foundation-wasm-ui.md) — Existing WASM UI capability model and boundaries.
- [`tauri-integration.md`](foundations/tauri-integration.md) — Tauri as host/platform layer.
- [`platform-synthesis.md`](foundations/platform-synthesis.md) — Synthesis: what `foundation_platform` should be.

## Plan
1. ~~Outline initial decisions.~~ ✅ 12 resolved.
2. ~~Create predocs/00-plan.md.~~ ✅
3. ~~Explore Basecamp Hotwire Native sources.~~ ✅
4. ~~Explore Tauri sources.~~ ✅
5. ~~Write foundation documents.~~ ✅ 4 documents.
6. ~~Create `foundation_platform` crate skeleton.~~ ✅
7. **Implement `foundation_platform` crate** (next)
   - Session backbone + navigation interception
   - Route handler trait + macro API surface
   - `ewe://` custom protocol registration
   - Transport lanes (command IPC, events, resources)
   - Capability registry
   - WebView profiles
   - Offline cache policy integration
8. **Integrate with WASM UI**
   - Wire session backbone into `foundation_wasm_ui` JS runtime
   - Route native-side and web-side sessions as peers
9. **Testing**
   - Unit tests for platform modules
   - Integration tests for Tauri-WASM UI bridge
   - Functional testing on desktop targets
