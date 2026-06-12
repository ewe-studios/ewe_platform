# Feature 20 — Status: COMPLETE (2026-06-12)

## What shipped

- **`wasm_bins` → `foundation_wasm::build_tools`** (feature `build-tools`:
  optional foundation_codegen/toml/toml_edit; std-gated inside the no_std
  crate via `extern crate std` + explicit hosted preludes) and its CLI →
  `foundation_wasm::cli` (feature `cli`, clap). The ABI crate owns how its
  entrypoints become wasm binaries.
- **`wasm_bundle` → `foundation_wasm_ui::build_tools`** (+ `cli` with the
  `wasm-bundle` command). The UI crate owns how its runtime ships.
- Features OFF by default — default builds of both runtime crates stay
  no_std/wasm-clean (workspace check green). Enabling them on wasm32 is a
  compile error by construction, which is correct: build tooling never
  belongs in the artifact.
- `foundation_codegentools` keeps the generic pieces (schema_gen, wasm FILE
  inspection CLI); moved unit tests + the wasm_crate/rlib_crate fixtures
  travelled with their code.
- `bin/platform` rewired: `wasm_bins` subcommand now from
  `foundation_wasm::cli`; NEW `wasm-bundle` subcommand from
  `foundation_wasm_ui::cli`.
- **`APACHE_ARROW_JS`** added to `foundation_wasm_ui::embedded` (the bundled
  `runtimes/apache-arrow.js`, G3/G25) under the existing `embedded-js`
  feature — the wire-v2 (real Arrow IPC) reader ships next to the runtimes;
  the wasm loop's compact v1 never loads it.

## Verification

foundation_wasm tests (41 + 6 + 4 + validator suites) green with `--features
cli`; foundation_wasm_ui suites green incl. the moved 9 bundle tests and the
new APACHE_ARROW_JS embed assertion; codegentools remaining suites green;
zero clippy on both runtime crates with cli features; full workspace check
green (default features untouched).
