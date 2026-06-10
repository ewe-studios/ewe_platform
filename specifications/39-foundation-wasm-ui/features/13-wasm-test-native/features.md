# Feature 13: Native `#[wasm_test]` discovery & result protocol

**Crates:** `foundation_macros` (the macro), `foundation_wasm` (result FFI), `foundation_wasm_testbed` (discovery)
**Decisions:** 031 (owned WASM infrastructure)
**Pairs with:** Feature 12 (testbed runners), Feature 14 (wasm-bindgen interop — the opt-in alternative)

---

## Goal

A fully owned wasm test-execution model — no wasm-bindgen — comprising: a `#[wasm_test]`
macro, a stable export convention the testbed discovers with `walrus`, and a result protocol
that flows pass/fail/panic back over the `foundation_wasm` ABI to the JS runner.

---

## `#[wasm_test]` macro (in `foundation_macros`)

Per [[feedback_macros_location]], the macro lives in `foundation_macros` (never a companion
`*_macros` crate).

```rust
#[wasm_test]
fn arrow_batch_round_trips() {
    let ops = vec![DomOp::SetText { node_id: 5, text: "hi".into() }];
    assert_eq!(ArrowEncoder.decode(&ArrowEncoder.encode(ops.clone())).unwrap(), ops);
}

#[wasm_test]
async fn async_yield_reschedules() { /* ... */ }
```

Expands to a `#[no_mangle] pub extern "C" fn __fwt_<name>()` that:
1. Catches panics at the boundary (`catch_unwind`) — a panic = a failed case, not a trap.
2. Runs the body (sync; async cases drive the owned executor / yield loop to completion).
3. Reports the outcome via the result FFI below.

It also contributes the case to a manifest export so the runner can enumerate without running.

### Discovery convention

- Exports are prefixed **`__fwt_`** (our prefix — mirrors how the bindgen path used `__wbgt_`,
  but ours and on our runtime).
- `__fwt_manifest` (a generated export, or a walrus prefix scan) yields the case names + flags
  (async, should_panic, ignore) so `foundation_wasm_testbed` lists/filters cases.
- Discovery uses `walrus` to read the export section of the *raw cargo wasm output* (no
  wasm-bindgen processing) — `walrus` here is a wasm-format reader, not a wasm-bindgen dep
  (decision 031).

---

## Result protocol (over the owned ABI)

Two viable mechanisms; pick one in implementation:

1. **`host_report` import** added to `web_abi`: `host_report(status: u32, ptr: u32, len: u32)`
   where status ∈ {pass=0, fail=1, ignored=2} and `(ptr,len)` is an optional UTF-8 message
   (assertion text / panic payload) in the arena. Simple, direct.
2. **A result protocol byte** shipped via `host_apply` (reuse the envelope/dispatch path) — fits
   the existing transport, no new import.

Either way: the JS runner registers a handler, collects per-case results, and prints a
`node:test`-style summary; the process exits non-zero if any case failed. Panics caught in the
export are reported as `fail` with the panic message.

### Async cases

Async `#[wasm_test]` bodies run on the owned valtron executor + JS yield loop (the same path
spec-31 feature-02 exercised, but without `#[wasm_bindgen_test]`). The case reports its result
when the future resolves; the runner awaits the report before the next case.

---

## What this replaces (spec-31)

- `#[wasm_bindgen_test]` → `#[wasm_test]`.
- `__wbgt_` discovery (wasm_test.rs) → `__fwt_` discovery (same walrus technique, our prefix).
- wasm-bindgen CLI glue → none; the module is loaded directly by `foundation-wasm.js`.

---

## Success criteria

- [ ] `#[wasm_test]` compiles a case into a discoverable `__fwt_` export; panics become failures.
- [ ] `foundation_wasm_testbed` enumerates cases via `walrus` (our prefix) without running them.
- [ ] Pass/fail/panic-message flow back over the owned ABI; runner prints a summary and sets exit code.
- [ ] Async cases resolve via the owned executor; no wasm-bindgen anywhere in the path.
- [ ] An example case in `integrations/nodejs/foundation-wasm/` runs green through this path.
