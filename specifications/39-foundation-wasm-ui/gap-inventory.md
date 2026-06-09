# Spec-39: Gap Inventory

**Date:** 2026-06-09
**Status:** All gaps resolved. F11 (Request Batching) added as new feature.

Comprehensive catalog of unresolved gaps across all 11 feature specs. Grouped by priority.
P0 gaps are blocking — no coding can start until these are resolved.

---

## Priority Legend

| Priority | Meaning |
|----------|---------|
| **P0** | Blocking — cross-feature, no coding can start |
| **P1** | Should fix before coding — would cause rework or bugs |
| **P2** | Implementation details — can be decided during coding |

---

## P0 Gaps (Blocking)

### G1. NodeRegistry Lifecycle (F01, F05, F06, F07, F08) — **RESOLVED**
**Resolution:** Added `RegisterNode` (op 17) and `UnregisterNode` (op 18) to `DomOp` enum (19 total).
`CreateElement`/`CreateTextNode` no longer auto-register — caller must emit `RegisterNode` explicitly.
`RemoveNode` implicitly unregisters. `ReplaceNode` implicitly unregisters old + registers new.
`NodeRegistry` JS class specified with `register()`, `unregister()`, `get()`, `clear()`, `size`.
Reserved node_ids 0-2 for `<head>`, `<body>`, `<html>`. Macro IDs start at 1000.

### G4. WASM Callback Data Shape (F02, F03, F08) — **RESOLVED**
**Resolution:** JS serializes full `EventData` (type, primalId, value, checked, keyCode, modifiers) into
an arena slot via `create_allocation()`. Passes `MemoryId` to `invoke_callback`. Rust reads from arena,
deserializes `EventData`, dispatches to callback, then deallocates the slot (no JS-side dispose needed).
Callback type changed from `Box<dyn FnMut(serde_json::Value)>` to `Box<dyn FnMut(EventData)>`.

### G5. Runtime Construction & Cross-Runtime Wiring (F00, F02, F04) — **RESOLVED**
**Resolution:** No bridge trait needed. Effects are closures — they capture whatever they need from
their environment. `DomSignalBinding::bind` (decision 004) captures `SignalGetter` and `InstructionReceiver`
directly. `AppOrchestrator` just holds both runtimes side by side; the developer or binding helper
wires them at the call site where both are in scope.

### G6. ProtocolHandler FFI Signature Mismatch (F00, F04) — **RESOLVED**
**Resolution:** All protocols use uniform 3-param FFI: `host_apply(mem_id, ptr, len)`. CustomBinary
packs ops and text pool into a single arena slot — payload starts with `[text_pool_offset: u32]
[text_pool_length: u32][ops_binary...][text_pool...]`. JS handler reads the offset, splits the
payload, applies both. `ProtocolMethods<T>` extends `ProtocolHandler`, calls `send_to_js()` which
uses the uniform FFI. One slot, one `dispose_allocation` for all protocols.

---

## P1 Gaps (Should Fix Before Coding)

### G2. Event Wiring Duplication (F06 ↔ F08) — **RESOLVED**
**Resolution:** Hydrator handles `<style primal:style>` and `<script primal:script>` ONLY. All `primal:on*`
event wiring is owned by EventRuntime (`scanAndWire()` + MutationObserver). After `Patcher.materialize()`,
calls `Hydrator.hydrate()` THEN `runtime.scanAndWire()`. MutationObserver may also fire — `scanAndWire()`
is idempotent (removes old listener before adding new). Island `disconnectedCallback` calls both
`_scope.cleanup()` and `EventRuntime.removeListeners()`.

### G31. MorphDom Static Mutable State (F07) — **RESOLVED**
**Resolution:** Per-morph state (idMap, persistentIds, oldIdTagMap, duplicates) moved to `MorphContext`
instance. Only `pantry` (hidden div) and `scripts` (WeakSet) remain static. `MorphDom.morph()` creates
a fresh `MorphContext` per call — re-entrant morphs can't corrupt each other's state.

### G19. mount(ctx) Integration Detail (F03) — **RESOLVED**
**Resolution:** `mount()` function specified: (1) prefix_ids, (2) queue_create_ops, (3) queue_register_ops,
(4) per-Part effect creation. Table shows Part→Effect mapping: Text/Attribute/Children get effects,
Event gets one-time AddEventListener. Macro generates closures that capture signal getters.

### G43. stabilize()/flush() Atomicity (F00) — **RESOLVED**
**Resolution:** `stabilize()` uses `try_flush()` and logs errors. If flush fails, signal state is already
updated but DOM may lag. Effects are idempotent — next `stabilize()` re-reads signals and re-emits
correct DomOps. No rollback needed. Signal system is source of truth, effects converge DOM naturally.

### G3. JS-side Arrow IPC Parsing — **RESOLVED**
**Resolution:** Apache Arrow JS library bundled directly into our `assets/` directory. We copy
the `apache-arrow` JS source file — no npm dependency for the end user. `import { tableFromIPC }
from './apache-arrow.js'` in our bundled runtime.

### G25. Arrow FlatBuffer Parsing — **RESOLVED** (same as G3)

---

## P2 Gaps (Decide During Coding)

### F00: G8 (internal_api module), G9 (non-WASM stubs), G10 (line references)
### F01: G11 (ProtocolEncoder generic scope), G12 (Html String allocation), G13 (Part ownership), G14 (DomOp String allocation)
### F02: G15 (RefCell thread safety), G16 (notification_manager ordering), G17 (callback_id allocation timing), G18 (ComputedNode downcast)
### F03: G20 (loop prefix allocation), G21 (MaybeCallback trait placement)
### F04: G22 (mem::take zero capacity)  — G24 resolved by SendResult change
### F05: G26 (MORPH_NODE ordering with other ops), G27 (Arrow IPC size limit)
### F06: G28 (Transport API inconsistency), G29 (_resolveTarget side effects), G30 (window.primal collision)
### F07: G32 (moveBefore browser support), G33 (pantry leak on error)
### F08: G35 (MutationObserver perf), G36 (WeakMap cleanup timing)
### F09: G37 (lightningcss Rust vs Node), G38 (reserved node_id values), G39 (ThemeTokens dark naming)
### F10: G40 (wasmtime binary size) — **RESOLVED**. No wasmtime needed. Proc macros expand to `#[wasm_entrypoint]`, `foundation_codegen` scans source with `syn`.
### F10: G41 (meta pointer reading) — **RESOLVED**. Same as G40 — metadata is in the `#[wasm_entrypoint]` attribute, no WASM instantiation needed.
### F10: G42 (workspace multi-crate) — **RESOLVED**. `ewe-wasm build --target <CRATE>` builds one crate. Multiple crates built separately.
### Cross-cutting: G7 (Transport vs proc macros), G44 (FFI name conflict), G45 (primal-id string↔number), G46 (SSE content-type), G47 (foundation_signals location), G48 (arrow crate features)

---

## Resolution Log

| Gap | Status | Resolution file | Date |
|-----|--------|-----------------|------|
| G1 | **Resolved** | F01 (DomOp +19 variants), F05 (NodeRegistry class, ops 17-18) | 2026-06-09 |
| G2 | **Resolved** | F06 (Hydrator = styles+scripts only, EventRuntime = events, idempotent scanAndWire) | 2026-06-09 |
| G4 | **Resolved** | F08 (EventData struct, arena-serialized callback), F02 (EventData callback type) | 2026-06-09 |
| G5 | **Resolved** | F00 (closure capture, no trait), F02 (DomSignalBinding matches decision 004), F03 (mount passes receiver) | 2026-06-09 |
| G6 | **Resolved** | F00 (uniform `host_apply` 3-param), F04 (ProtocolMethods extends ProtocolHandler, single slot) | 2026-06-09 |
| G19 | **Resolved** | F03 (mount function with per-Part effect table, queue_create_ops, queue_register_ops) | 2026-06-09 |
| G31 | **Resolved** | F07 (MorphContext instance-level state, only pantry+scripts static) | 2026-06-09 |
| G34 | **Resolved** | Same as G4 (EventData struct carries full event context) | 2026-06-09 |

---

_Created: 2026-06-09_
