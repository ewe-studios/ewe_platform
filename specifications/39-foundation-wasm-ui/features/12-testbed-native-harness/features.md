# Feature 12: Testbed-Owned Native WASM Test Harness

**Crate:** `foundation_wasm_testbed` (extended)
**Runtime:** `foundation-wasm.js` + `foundation-wasm-ui.js` (this spec's owned runtime)
**Decisions:** 014 (execution modes), 016 (build pipeline)
**Builds on / supersedes:** `specifications/completed/31-wasm-testbed`

---

## Goal

`foundation_wasm_testbed` owns the full test loop for `foundation_wasm`-based wasm32 modules,
running them under our OWN runtime with **zero wasm-bindgen and zero wasm-pack**:

- Build a `foundation_wasm` cdylib to `wasm32-unknown-unknown` (LLVM backend).
- Stage it into a harness directory as a fixture.
- Run the JS suite (`node`, `deno`, or browser) that loads `foundation-wasm.js` and instantiates
  with `{ abi: runtime.web_abi }`.
- Discover and run tests via OUR convention; collect pass/fail over OUR ABI.
- Provide CLI commands so contributors run `wasm-testbed <cmd>` instead of hand-rolling
  `cargo build … && node --test`.

---

## Spec-31 gap analysis (what was not properly done)

| Spec-31 mechanism | Problem | This feature's owned replacement |
|---|---|---|
| `bindgen-web/deno/wrangler` modes run the `wasm-bindgen` CLI to generate JS glue | Pulls in wasm-bindgen as the interop layer; we own interop via `foundation_wasm` | `node/deno/web` modes load `foundation-wasm.js`; no glue generation — the module exports the raw `foundation_wasm` ABI |
| Test discovery via `__wbgt_` exports (walrus) — the wasm-bindgen test convention | Couples discovery to wasm-bindgen's naming | OUR convention: a `#[wasm_test]` macro (foundation_macros) exporting `__fwt_*` functions, discovered via walrus by OUR prefix |
| Feature-02 wrote `#[wasm_bindgen_test]` tests, wasm-pack-style | Tests can't run without wasm-bindgen | Rewrite as `#[wasm_test]` running on `foundation-wasm.js` |
| `wasm.rs` / `wasm_test.rs` exist solely for wasm-bindgen glue + `__wbgt_` discovery | Dead weight once we own the path | Keep ONLY as an optional `--interop=wasm-bindgen` escape hatch (not the default); native path is default |
| README/requirements list `wasm-bindgen` as a core tool | Signals the wrong ownership | Native harness needs only `cargo` + a JS runtime (node/deno) + (optional) Playwright for browser |

**Note:** wasm-bindgen interop support need not be deleted — but it must become an explicit,
non-default mode. Our own crates test through the owned harness.

---

## Owned architecture

```
┌────────────────────────────────────────────────────────────────────┐
│ Rust test crate (cdylib, std)                                       │
│   #[wasm_test] fn my_case() { assert!(...); fwt::report(...) }       │
│   → exports __fwt_my_case (+ a __fwt_manifest listing cases)         │
│   deps: foundation_wasm (feature "web"), foundation_wasm_ui          │
└───────────────────────────┬─────────────────────────────────────────┘
                            │ cargo build --target wasm32 (LLVM backend)
┌───────────────────────────▼─────────────────────────────────────────┐
│ foundation_wasm_testbed                                              │
│   build.rs   : cargo → wasm32 (CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm)│
│   discover   : walrus scan for __fwt_* exports (OUR prefix)          │
│   stage      : copy .wasm into harness fixtures/                     │
│   run        : node | deno | browser, loading foundation-wasm.js     │
│   report     : parse results, exit non-zero on failure              │
└───────────────────────────┬─────────────────────────────────────────┘
                            │ instantiate { abi: rt.web_abi }
┌───────────────────────────▼─────────────────────────────────────────┐
│ foundation-wasm.js (+ foundation-wasm-ui.js)                         │
│   runs each __fwt_ export; the module reports pass/fail back over    │
│   the ABI (a result protocol byte, or a host_report import)          │
└──────────────────────────────────────────────────────────────────────┘
```

### Test-discovery + reporting convention (OUR own)

- **`#[wasm_test]`** proc macro (in `foundation_macros`): wraps a `fn()` (or `async fn`),
  emitting a `#[no_mangle] extern "C" fn __fwt_<name>()` plus registering it in a manifest.
- **Manifest export** `__fwt_manifest` (or walrus prefix scan) lets the testbed enumerate cases by
  name without running them — mirrors how the bindgen path used `__wbgt_`, but on our prefix and
  our runtime.
- **Reporting**: a thin `host_report(status, msg_ptr, msg_len)` import (added to `web_abi`), OR
  reuse `host_apply` with a dedicated result protocol. Panics are caught at the export boundary and
  reported as failures. JS aggregates and prints a `node:test`-style summary.

### Build step (shared with feature 10)

Reuse `foundation_wasm_testbed::build` — it already sets `CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm`
so wasm32 builds despite the repo's Cranelift `dev` default. Feature 10's `ewe-wasm build`
(production bundling) and this harness share that wasm-build primitive; factor it so both call one
place.

---

## CLI commands (additions to `foundation_wasm_testbed`)

| Command | Behaviour |
|---|---|
| `wasm-testbed node <crate>` | Build crate → wasm32, stage into the node harness, run `node --test`, report. Default owned mode. |
| `wasm-testbed deno <crate>` | Same, run under Deno (headless wasm, no browser). |
| `wasm-testbed web <crate> [--headless]` | Same, serve + run under Playwright using our runtime (no bindgen glue). |
| `wasm-testbed init node <crate>` | Scaffold a node harness (package.json type=module, mock-wasm/mock-dom, a sample `#[wasm_test]`). |
| existing `test bindgen-*` | Retained behind an explicit opt-in; documented as interop-only, not for our crates. |

These commands subsume the manual `integrations/nodejs/foundation-wasm/build-module.sh` +
`node --test` flow proven this session.

---

## Migration plan

1. Land the owned node runner + `build`/`stage`/`run` plumbing in `foundation_wasm_testbed`
   (reusing the existing `build`/`deno` modules; add a `node` module).
2. Add `#[wasm_test]` + `__fwt_` discovery (walrus) + the result protocol.
3. Port spec-31 feature-02's JS-yield integration tests from `#[wasm_bindgen_test]` to `#[wasm_test]`.
4. Re-label spec-31's `wasm-bindgen` tool dependency as optional-interop; update its requirements
   to reflect the owned default.
5. Grow the harness alongside feature 00 / Task 5 (each new JS class — EventDispatcher, SignalBridge,
   ComponentRegistry — gets owned-harness coverage).

---

## Proven seed (already in-tree, this spec)

- `integrations/nodejs/foundation-wasm/module/` — standalone `std` cdylib exporting `emit_arrow_batch`
  (real `foundation_wasm` ABI; imports only `abi.host_apply`).
- `backends/foundation_wasm/runtime/foundation-wasm.js` + `backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js`.
- `integrations/nodejs/foundation-wasm/test/*.test.js` — 15 `node:test` cases, incl. a real-module
  WASM→JS→DOM e2e, all with **no wasm-bindgen / wasm-pack**.
- `build-module.sh` builds with `--profile uat` (LLVM); the testbed `build` module already does the
  equivalent via the `CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm` env override.

This feature turns that proof into testbed-owned commands + a discovery/reporting convention.

---

## Success criteria

- [ ] `wasm-testbed node <crate>` builds a `foundation_wasm` cdylib, stages it, runs the node suite, exits non-zero on failure — no wasm-bindgen/wasm-pack invoked.
- [ ] `#[wasm_test]` macro + `__fwt_` discovery enumerate and run cases on `foundation-wasm.js`.
- [ ] Pass/fail (and panics) reported back over our ABI and summarised by the runner.
- [ ] Deno runner works headlessly; browser runner works via Playwright on our runtime.
- [ ] Spec-31 feature-02 JS-yield tests re-expressed as `#[wasm_test]` and green via the owned harness.
- [ ] wasm-bindgen path retained only as explicit `--interop=wasm-bindgen` opt-in.
- [ ] The shared wasm-build primitive (LLVM backend) is factored so feature 10 and feature 12 use one implementation.
