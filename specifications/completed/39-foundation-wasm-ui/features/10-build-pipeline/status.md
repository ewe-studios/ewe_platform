# Feature 10 — Status: COMPLETE (2026-06-12)

## What shipped

- **Mode macros** (`foundation_macros::wasm_modes`): `#[wasm_bin]`,
  `#[wasm_worker]`, `#[wasm_service]` — marker-validation macros (js =
  single-file|separate, encoded = b64|uint8array gated on single-file, desc,
  routes required on wasm_service and rejected elsewhere). They emit the fn
  unchanged: the pipeline reads attributes from SOURCE via `CrateScanner`, so
  no textual expansion to `#[wasm_entrypoint]` is needed (the spec's expansion
  diagram describes intent; marker validation delivers it with less
  machinery — documented deviation).
- **`foundation_codegentools::wasm_bundle`**: `WasmBundleGenerator` wrapping
  the existing `WasmBinGenerator` — scans all three mode attributes, maps
  metadata to `BundleEntrypoint { mode, packaging, routes }`, `plan()`
  dry-run, `execute()` = inner generate → `cargo build --target wasm32`
  (release, or the uat profile for dev — wasm32 needs LLVM, the workspace dev
  profile is Cranelift) → per-mode wrappers → bundles → runtime assets.
- **`js_wrapper`** (pure template fns, §4): bin wrapper (owned runtime
  `rt.web_abi` instantiation + entrypoint invoke), worker pair (ready
  handshake + host with send/terminate), service worker (skipWaiting/claim,
  route-prefix fetch interception per decision 017). One `__EWE_WASM_LOAD__`
  seam shared with the bundler.
- **`bundler`** (§5): `Uint8Array` literal or base64+atob embedding (no
  base64 crate — 20 lines), replacing the fetch path entirely.
- **CLI**: `wasm-bundle build|plan` subcommand (registered in `cli`):
  `--release`/`--dev` conflict enforced by clap, `--output`, `--target`,
  `--skip-runtimes`, `--verbose`.

## Verification

4 in-macro validation tests (valid forms, routes required, encoded⇒single-
file, bad values incl. routes on non-service) + 9 wasm_bundle tests: wrapper
structure per mode (spec 9-11), both bundle forms self-contained with no
fetch (12-14), base64 padding edges, attribute→entrypoint mapping (defaults,
single-file+b64, routes list), CLI flag conflict (3) and full flag parse
(1/2/4/5 surface). All codegentools + macros suites green; zero clippy.

## Deferred (documented)

Integration tests 15-18 (full cargo pipeline, live worker round-trip, SW
interception) need a browser/worker host and a real wasm compile — the
wrapper templates and generator orchestration they exercise are covered
above; run them via the web testbed when an app crate adopts the pipeline.
