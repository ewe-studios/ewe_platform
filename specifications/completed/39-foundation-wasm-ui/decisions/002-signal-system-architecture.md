# 002 — Signal system: standalone `foundation_signals` crate with R3-style architecture

**Date:** 2026-06-08
**Status:** Resolved

### Decision

Create a standalone `foundation_signals` crate — pure Rust, no DOM, no WASM, no UI coupling. Any project that needs fine-grained reactive signals can use it.

A **JavaScript counterpart** lives in `foundation-wasm-ui.js` (or its own JS file) — implementing the same signal semantics on the client side for JS-only reactive state.

### Architecture (R3-style, from learnings)

**Not the simple `Arc<Mutex<T>> + Vec<SubscriberId>` approach.** The signal system uses:

- **Height-based topological ordering** — each computed has `height = max(dep heights) + 1`, processed level-by-level → glitch-free
- **Bucket queue** — `dirtyHeap[height]` = linked list of dirty computeds → O(1) insertion, level-by-level processing
- **Version-based stale detection** — global `version` counter, per-link `version_` → dynamic dependency tracking
- **`startTracking` / `endTracking`** — re-evaluation discovers new deps, removes stale ones automatically
- **Diamond dependency safety** — height ordering guarantees each computed processes once, no special detection needed
- **Explicit disposal** — `onCleanup()` callback runs before recompute and when unwatched (for timers, DOM listeners, etc.)
- **Batch coalescing via `stabilize()`** — multiple `set()` calls → one effect flush → one Arrow batch → one JS `host_batch_apply()`
- **Three-state flags** — Clean, Check, Dirty → enables short-circuit evaluation and lazy updates

### What this replaces

- Discards old_features/02's simple `Arc<Mutex<T>> + Vec<SubscriberId>` design — no dependency graph, no glitch-freedom
- Discards the `Computed` with simple version cache — no dynamic deps, no diamond safety

### Tradeoffs

- ~500-800 lines vs ~150 lines for the simple version, but well-understood from R3/Datastar source code
- Generic over `T` — works in any Rust context, not tied to DOM/WASM
- JS counterpart mirrors the same semantics (without Proxies — path-based store for nested state)
