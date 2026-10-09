# Feature 14: wasm-bindgen interop boundary (minimal, opt-in)

**Crates:** `foundation_wasm_testbed` (opt-in modes), integrating crates (`worker-rs` users, `foundation_db`)
**Decisions:** 031 (owned WASM infrastructure)
**Counterpart to:** Feature 13 (the owned default)

---

## Goal

Define the **only** sanctioned wasm-bindgen usage and keep it as small and isolated as
possible. Everything else is owned (decision 031). wasm-bindgen is an *integration adapter*,
never infrastructure.

---

## Where wasm-bindgen is allowed

| Integration | Why wasm-bindgen is unavoidable | Boundary |
|---|---|---|
| **Cloudflare Workers (`worker-rs`)** | `worker-rs` is built on wasm-bindgen; the Workers runtime expects its glue | Confined to the worker entrypoint crate; our app logic stays on the owned ABI and is called across a thin shim |
| **`foundation_db` / external SDKs** | Ship wasm-bindgen bindings we don't control | Confined to the integrating module; converted to owned types at the seam |
| **Testbed `bindgen-*` modes** | Useful for testing crates that themselves use wasm-bindgen | Retained behind explicit `--interop=wasm-bindgen`; NOT the default, NOT for our crates |

## Where wasm-bindgen is NOT allowed

- The shared runtime (`foundation_wasm`, `foundation-wasm*.js`).
- The default build pipeline (F10) and the default test harness (F12/F13).
- Anything spec-39 ships as "the framework."

---

## Rules

1. **Opt-in only.** Default everything is owned. wasm-bindgen requires an explicit flag/feature
   (`--interop=wasm-bindgen`, a crate feature, or living in a clearly-named integration crate).
2. **Isolated.** wasm-bindgen types/glue never cross into the shared runtime; they're converted to
   owned types (`foundation_wasm` ABI values, `DomOp`, etc.) at the boundary.
3. **Minimal surface.** The integration shim is as small as possible and documented as integration,
   not infrastructure.
4. **`walrus` is fine** for parsing wasm-bindgen's output at these boundaries (and for our own
   `__fwt_` discovery in F13) — it's a wasm-format tool, not a wasm-bindgen dependency.

## Testbed changes

- Keep `wasm::run_wasm_bindgen` and `wasm_test::discover_tests` (`__wbgt_`) but reachable only via
  the explicit opt-in mode; surface a clear log line that an interop (non-owned) path is in use.
- Default `node`/`deno`/`web` runners (F12) never touch wasm-bindgen.

## Success criteria

- [ ] wasm-bindgen is reachable only via explicit opt-in; no default path invokes it.
- [ ] A Cloudflare `worker-rs` example interops via a thin shim while app logic stays on the owned ABI.
- [ ] `foundation_db` (or a representative SDK) integration converts to owned types at the seam.
- [ ] Testbed clearly distinguishes owned runs from opt-in interop runs.
- [ ] No wasm-bindgen symbol/type appears in `foundation_wasm`, `foundation_wasm_ui`, or the
      `foundation-wasm*.js` runtime.
