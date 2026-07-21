# Incidents — foundation_wasm_ui

## 001 — Local arena silently dropped all WASM DomOps

**Date:** 2026-07-19
**Severity:** Critical — WASM client mode completely non-functional
**Affected:** `App::with_protocol()`, `App::new()`, `App::columnar()`, `App::json()`, `App::arrow()`, all presets that use `Runtime::builder().memory(MemoryAllocations::new())`

### What happened

`App::with_protocol()` built the `Runtime` with `.memory(MemoryAllocations::new())` — a
private, receiver-owned arena. The `InstructionReceiver` allocated message slots in this
private arena during `stabilize()`, then shipped the slot id to JS via `host_apply()`.

JS's `host_apply` handler (`foundation-wasm.js` line ~1767) dispatches the protocol,
then ACKs by calling `exports.dispose_allocation(memory_id)` — a WASM export that operates
on the **global** `ALLOCATIONS` static (`static ALLOCATIONS: Mutex<MemoryAllocations>`).

The slot id was allocated in a **private** arena but freed in the **global** arena.
The global arena's generation check failed — the id simply didn't exist there.

### What broke

Every `App::stabilize()` in WASM client mode:
1. Allocated a slot in the private arena (succeeded)
2. Encoded DomOps into that slot (succeeded)
3. Called `host_apply(memory_id, ptr, len)` (succeeded)
4. JS dispatched the protocol and ACKed via `dispose_allocation(memory_id)`
5. **`dispose_allocation` panicked or silently no-op'd** — slot id not found in global arena

Result: DomOps either caused a WASM trap (visible in JS console) or were silently
discarded depending on error handling. Nothing rendered.

### What did NOT break (why it went unnoticed)

**`App::mock()`** — tests use protocol byte 255 (`MockProtocol`), which records batches
directly into an `Rc<RefCell<Vec<Vec<DomOp>>>>` and **never calls `host_apply`**. The
private ACK path (`receiver.ack(memory_id)`) works fine with any arena because it routes
through the same arena that allocated the slot.

All 18 Rust tests passed. The JS integration tests exercise the protocol handlers
directly (they feed bytes to `DomOpApplicator`), bypassing the `host_apply` →
`dispose_allocation` loop entirely.

### The fix

Changed `App::with_protocol()` to use `.global_arena()` instead of `.memory(..)`:

```rust
// Before (broken for WASM mode):
let runtime = Runtime::builder()
    .protocol(protocol)
    .memory(MemoryAllocations::new())  // private arena → host_apply ACK mismatch
    .build();

// After (correct for WASM mode):
let runtime = Runtime::builder()
    .protocol(protocol)
    .global_arena()                     // same arena that dispose_allocation operates on
    .build();
```

The `global_arena()` path uses `internal_api::with_global_allocations()` — exclusive
access to the same `ALLOCATIONS` static that the `exposed_runtime::dispose_allocation`
export operates on. Slot allocation and slot disposal now target the same arena.

### Files changed

| File | Change |
|------|--------|
| `src/app.rs` | `.memory(MemoryAllocations::new())` → `.global_arena()` in `App::with_protocol()` |
| `runtimes/foundation-wasm-ui.js` | Added `registerWasmApp(runtime)` — wires protocol byte 1 (columnar) with `NodeRegistry` + `DomOpApplicator` + `EventDispatcher`, which WASM mode also lacked |
| `src/build_tools/js_wrapper.rs` | Generated `init()` wrapper auto-calls `registerWasmApp(rt)` so users just call `await init()` |

### Root cause

The `Arena` enum in `receiver.rs` documents the constraint explicitly (lines 23-26):

> The live JS loop MUST use `Arena::Global`: JS ACKs by calling the `dispose_allocation`
> WASM export, which frees the GLOBAL arena — a slot id from a private arena would fail
> its generation check there (the feature-00 arena seam).

`App::with_protocol()` used `Arena::Owned` (the "native tests / custom hosts" path)
despite being the constructor for the live WASM loop. The two constructors existed
(`new` + `with_global_arena`) but the caller picked the wrong one.

### Lessons

1. **Cross-boundary resources need a single authority.** When Rust allocates an id that
   JS must later free, both sides MUST share the same arena. A `MemoryAllocations::new()`
   creates a parallel universe that JS has no idea about.

2. **Mock tests don't exercise the FFI boundary.** `App::mock()` bypasses `host_apply`
   entirely, so it can't catch arena mismatches. Any code path that crosses
   `host_apply`, `host_invoke_function`, or similar WASM imports needs a test that
   actually crosses the boundary (either a real browser test or a native integration
   test that wires the exposed_runtime exports).

3. **The `with_global_arena()` constructor existed but wasn't used.** The API had the
   right tool; `App` just reached for the wrong one. When a constructor name says
   "for tests" and another says "for the live loop", the default should be the live one.

4. **Silence is not success.** No error surfaced in tests, no warning in the build.
   The only symptom was a blank browser page — which looked like "WASM isn't loaded"
   rather than "arena id mismatch on dispose."
