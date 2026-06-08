# Spec-39: Gap Analysis & TODOs

**Date:** 2026-06-08

Comprehensive catalog of everything still unresolved after decisions 001-010. Old features and primal-ui.md reviewed, resolved items crossed out, gaps grouped by theme.

---

## Already Resolved (decisions 001-010)

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

---

## Critical Unresolved (Blocks Implementation)

### 1. Web Component Architecture
**Source:** `old_features/05-web-component-base/feature.md`, lines 3-19  
**Question:** Generic inheritable `CustomComponent` class? How do components map to WASM execution modes (direct, ServiceWorker, WebWorker)? How is mode negotiated, identified, and lifecycle managed?

### 2. Three WASM Execution Modes
**Source:** `old_features/05-web-component-base/feature.md`, lines 7-17; `requirements.md`, lines 636-644  
**Question:** 
- Direct calls vs ServiceWorker (HTTP interception) vs WebWorker (postMessage)
- How does the component know which mode it's running in?
- How does communication differ between modes?
- Can the same component run in any mode transparently?
- `#[wasm_bin]` vs `#[wasm_worker]` vs `#[wasm_service]` proc macros?

### 3. Protocol Abstraction & Version Negotiation
**Source:** `old_features/06-js-runtime-core/feature.md`, lines 3-7; `old_features/04-dom-batching-arrow/feature.md`, line 9; `requirements.md`, line 634  
**Question:**
- `u8 protocol` + `u8 version` in all messages?
- Trait-based protocol handler system?
- How JS selects protocol on initialization?
- How Rust routes to correct protocol handler?

### 4. Primal-UI Core Integration (`<mount-ui>`, `<mount-data>`, `<mount-stream>`)
**Source:** `primal-ui.md`, lines 205-519; `requirements.md`, line 632  
**Question:**
- How do `<mount-ui api="/v2/users">` elements work under the hood?
- MutationObserver-based discovery (Stimulus-style scanning)?
- Transport-agnostic streaming (SSE/WebSocket/chunked HTTP)?
- Transport negotiation flow?
- Server-driven signal updates — how does server know client state?

### 5. Server-Client State Synchronization
**Source:** `primal-ui.md`, line 649; `old_features/06-js-runtime-core/feature.md`, lines 393-420  
**Question:**
- Does client send full state or just changes with each request?
- State serialization format (JSON? Arrow? Binary?)?
- Selective state sync — every signal (Datastar) or only marked ones?
- Request bundling — Livewire-style microtask queue?

---

## Important Design Gaps (Should Address Before Coding)

### 6. DOM Morphing Strategy
**Source:** `primal-ui.md`, line 650  
**Question:** morphdom vs idiomorph vs custom? How does form state preservation integrate with Arrow batch system?

### 7. Fetch Wrapper & Request Bundling
**Source:** `requirements.md`, lines 635, 647; `primal-ui.md`, line 647  
**Question:**
- Where does it live? (`foundation-wasm.js` vs `foundation-wasm-ui.js`)
- Microtask queue vs timeout coalescing?
- How does server respond to bundled requests?

### 8. Event Attachment (`primal:onclick`)
**Source:** `primal-ui.md`, lines 30-57  
**Question:** How does `primal:onclick="controller.method"` resolve and wire up? MutationObserver + event delegation?

### 9. Scoped Script/Style Tags
**Source:** `primal-ui.md`, lines 68-140  
**Question:** `<script scoped primal:script>` and `<style scoped primal:style>` — supported or discarded? Atomic CSS StyleManager?

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

### 15. Signal Crate Separation
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
| 2 | Crate split (foundation_wasm_ui from foundation_wasm) | None |
| 3 | Protocol abstraction + version negotiation | 2 |
| 4 | JS runtime core (split SDK) | 2, 3 |
| 5 | Signal system (foundation_signals) | 1 |
| 6 | Arrow DOM batching | 1, 3 |
| 7 | DOM morphing + form preservation | 4, 6 |
| 8 | Web component base (3 execution modes) | 3, 4, 5, 6 |
| 9 | Headless UI components | 2, 5, 8 |
| 10 | Auth UI package | 2, 5, 8, 9 |
