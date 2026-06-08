# Spec-39: Gap Analysis & TODOs

**Date:** 2026-06-09

Comprehensive catalog of everything still unresolved after decisions 001-030. Old features and primal-ui.md reviewed, resolved items crossed out, gaps grouped by theme.

---

## Already Resolved (decisions 001-029)

| # | Decision | File |
|---|----------|------|
| 001 | `html!` macro: single, pure Rust, uses `foundation_html` parser | `decisions/001-html-macro-pure-rust.md` |
| 002 | Signal system: standalone `foundation_signals` with R3 architecture | `decisions/002-signal-system-architecture.md` |
| 003 | Context-based scoping: unified Runtime, sub-graphs for ownership | `decisions/003-context-based-scoping.md` |
| 004 | Bindings are Effects in the signal graph | `decisions/004-bindings-are-effects.md` |
| 005 | Part-level granularity with compile-time template IDs | `decisions/005-element-id-and-part-granularity.md` |
| 006 | Runtime-prefixed element IDs (`component:template` format) | `decisions/006-runtime-prefixed-element-ids.md` |
| 007 | IntoHtml: implement for all common types | `decisions/007-intohtml-all-types.md` |
| 008 | Initial effect execution queues DOM op | `decisions/008-initial-effect-queues-dom.md` |
| 009 | No backpressure concern (single batch + streaming) | `decisions/009-no-backpressure-concern.md` |
| 010 | Arrow batch: operation mapping and schema | `decisions/010-arrow-batch-schema.md` |
| 011 | Signal identity: no string keys | `decisions/011-no-string-keys.md` |
| 012 | `foundation_ui_traits` crate for shared types | `decisions/012-foundation-ui-traits.md` |
| 013 | Web Component: 3 execution modes (transport abstraction) | `decisions/013-web-component-execution-modes.md` |
| 014 | `#[wasm_bin]` vs `#[wasm_worker]` vs `#[wasm_service]` design | `decisions/014-execution-modes-design.md` |
| 015 | Crate split: foundation_wasm / foundation_wasm_ui | `decisions/015-crate-split.md` |
| 016 | Build pipeline | `decisions/016-build-pipeline.md` |
| 017 | Service Worker uses webserve | `decisions/017-service-worker-uses-webserve.md` |
| 018 | Event Runtime: direct binding + MutationObserver cleanup (non-island only) | `decisions/018-event-runtime.md` |
| 019 | Scoped script/style tags (compile-time transform, island owns scoped content) | `decisions/019-scoped-script-style-tags.md` |
| 020 | Theme system | `decisions/020-theme-system.md` |
| 021 | `<island>` web component: scoped content container | `decisions/021-island-web-component.md` |
| 022 | Content type handling | `decisions/022-content-type-handling.md` |
| 023 | Mount components | `decisions/023-mount-components.md` |
| 024 | Component class structure | `decisions/024-component-class-structure.md` |
| 025 | Server-client state sync | `decisions/025-server-client-state-sync.md` |
| 026 | Fetch bundling | `decisions/026-fetch-bundling.md` |
| 027 | DOM morphing | `decisions/027-dom-morphing.md` |
| 028 | WASM boundary memory management and buffer ownership | `decisions/028-wasm-memory-ownership.md` |
| 029 | Signals return (getter, setter); two-way bindings via compile-time registered callbacks | `decisions/029-signal-getter-setter.md` |
| 030 | Runtime-owned `InstructionReceiver` replaces global `FRAME_BATCH` | `decisions/030-instruction-receiver.md` |

---

## Critical Unresolved (Blocks Implementation)

### 1. Web Component Architecture ✅ RESOLVED

**Resolved via Decisions 013, 014, 021.** Islands handle their own lifecycle. MutationObserver handles non-island content only.
**Source:** `old_features/05-web-component-base/feature.md`, lines 3-19

### 2. Three WASM Execution Modes ✅ RESOLVED

**Resolved via Decisions 013, 014.** `#[wasm_bin]` / `#[wasm_worker]` / `#[wasm_service]` proc macros handle transport abstraction.
**Source:** `old_features/05-web-component-base/feature.md`, lines 7-17; `requirements.md`, lines 636-644

### 3. Protocol Abstraction & Version Negotiation ✅ RESOLVED

**Resolved via Decisions 014, 015, 022, 028.** Each protocol owns its contract, demuxed by protocol byte.
**Source:** `old_features/06-js-runtime-core/feature.md`, lines 3-7; `old_features/04-dom-batching-arrow/feature.md`, line 9; `requirements.md`, line 634

### 4. Primal-UI Core Integration ✅ RESOLVED

**Resolved via Decisions 021, 023, 024, 029.** Islands, mount-data, mount-stream, getter/setter signals.
**Source:** `primal-ui.md`, lines 205-519; `requirements.md`, line 632

### 5. Server-Client State Synchronization ✅ RESOLVED

**Resolved via Decision 025 (client sends actions only, server-rendered HTML is self-contained, no hydration needed) and Decision 029 (signals return getter/setter tuples, two-way bindings via compile-time registered callbacks).**
**Source:** `primal-ui.md`, line 649; `old_features/06-js-runtime-core/feature.md`, lines 393-420

---

## Important Design Gaps (Should Address Before Coding)

### 6. DOM Morphing Strategy ✅ RESOLVED

**Resolved via Decision 027.** `MorphDom` class based on Datastar's approach (morphdom + idiomorph + form preservation).
**Source:** `primal-ui.md`, line 650  
**Question:** morphdom vs idiomorph vs custom? How does form state preservation integrate with Arrow batch system?

### 7. Fetch Wrapper & Request Bundling ✅ RESOLVED

**Resolved via Decision 026.** Bundling auto-enabled for full-stack builds, manual config for JS-only.
**Resolved via Decision 030.** `InstructionReceiver` on `Runtime` handles batching + protocol dispatch — no handoff ambiguity.
**Source:** `requirements.md`, lines 635, 647; `primal-ui.md`, line 647

### 8. Event Attachment (`primal:onclick`) ✅ RESOLVED

**Resolved via Decisions 018, 029.** Direct binding with MutationObserver cleanup (non-island only). Setters registered as compile-time callbacks for two-way bindings.
**Source:** `primal-ui.md`, lines 30-57

### 9. Scoped Script/Style Tags ✅ RESOLVED

**Resolved via Decisions 019, 021.** Compile-time CSS transformation with `:parent` pseudo-class. `<island>` owns all scoped content lifecycle.
**Source:** `primal-ui.md`, lines 68-140

### 10. Feature Ordering

**Source:** `old_features/07-auth-ui-package/feature.md` (line 3); `old_features/08-headless-ui-components/feature.md` (line 3)  
**Question:** Headless UI before Auth UI? Auth UI last? Formalize dependency graph.

### 11. JS Runtime Delivery
**Source:** `primal-ui.md`, lines 531-534  
**Question:** foundation-wasm-ui.js bundles everything — user doesn't care about internals? Standalone vs ES modules?

---

## Lower Priority (Can Defer)

### 12. Animation Frame Hook Location
**Source:** `requirements.md`, lines 68, 95  
**Note:** Implicitly moves to `foundation_wasm_ui` (DOM-specific). Edge case: not available in workers/service workers.

### 13. String Caching (`host_cache_string`)
**Source:** `requirements.md`, line 138  
**Note:** Implicitly stays in `foundation_wasm` (general optimization, not DOM-specific).

### 14. TypeScript Binding Generation
**Source:** `requirements.md`, line 633  
**Note:** Tooling enhancement, not blocking. What gets generated? Types? Wrappers? Component stubs?

### 15. TypeScript Binding Generation ✅ DEFERRED

**Deferred.** JS wrapper types are generated by the build pipeline (decision 016). TypeScript types can be derived from that. No separate feature needed.
**Source:** `old_features/02-signal-system/feature.md`, line 78  
**Note:** ✅ Resolved in decision 002 — standalone `foundation_signals` crate.

### 16. Send + Sync on Computed Closures
**Source:** `old_features/02-signal-system/feature.md`, line 59  
**Note:** Minor — determined by whether Runtime is `Send + Sync` (WASM is single-threaded, native may differ).

---

## Feature Ordering (Proposed)

Based on dependencies, the working order should be:

| Phase | Feature | Depends On |
|-------|---------|------------|
| 1 | `foundation_ui_traits` (shared types: IntoHtml, Html, Part, DomOp) | None |
| 2 | Signal system (`foundation_signals`) | 1 |
| 3 | `html!` macro | 1 |
| 4 | `InstructionReceiver` | 1, 2 |
| 5 | Arrow encoding (17 ops + MORPH_NODE) | 1, 4 |
| 6 | JS runtime core (`foundation-wasm.js`) | None |
| 7 | JS runtime UI (`foundation-wasm-ui.js`) | 6 |
| 8 | Web components (island, mount-data, mount-stream) | 7 |
| 9 | DOM morphing (MorphDom) | 5, 7 |
| 10 | Event runtime (binding, MutationObserver) | 7 |
| 11 | Scoped styles + theme system | 3, 7 |
| 12 | Build pipeline CLI (`ewe-wasm build`) | 6, 7 |
| 13 | Fetch wrapper & request bundling | 7, 8 |
| 14 | Service worker routing (WebServe) | 7, 12 |

### Phase groupings

| Phase | Features | Focus |
|-------|----------|-------|
| Foundation (Rust) | 01, 02, 03, 04, 05 | Types, signals, macro, batching, encoding |
| JS Runtime | 06, 07 | foundation-wasm.js + foundation-wasm-ui.js |
| Web Components & DOM | 08, 09, 10 | Islands, mounts, morphing, events |
| Styling | 11 | Scoped CSS, theme generation |
| Build & Transport | 12, 13, 14 | CLI, fetch bundling, service worker |
