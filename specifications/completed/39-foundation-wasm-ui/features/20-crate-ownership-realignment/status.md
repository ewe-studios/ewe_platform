# Feature 20 — Status: COMPLETE (2026-06-12)

## What shipped

- **`wasm_bins` → `foundation_wasm::build_tools`** + CLI →
  `foundation_wasm::cli`. The ABI crate owns how its entrypoints become wasm
  binaries.
- **`wasm_bundle` → `foundation_wasm_ui::build_tools`** (+ `cli` with the
  `wasm-bundle` command). The UI crate owns how its runtime ships.
- **Target-gated, not feature-gated** (review decision): build tooling is a
  std, native-only concern, so the deps live under
  `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` and the modules
  behind `#[cfg(not(target_arch = "wasm32"))]` (gated `extern crate std` +
  explicit hosted preludes). Every native build carries the CLI — no flags
  to remember — while wasm32 builds stay no_std-clean by construction:
  build tooling never belongs in the artifact.
- **Standalone binaries** `ewe-wasm-bins` / `ewe-wasm-bundle` (src/bin/),
  built on every native build; on wasm32 they degrade to stub mains (cargo
  compiles all targets). Logs go through `tracing` (fmt subscriber in the
  bins, `tracing::error!` on failure, `tracing::info!` for generate/build
  narration); the `list`/`plan` dry-run reports stay on stdout as command
  output.
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

foundation_wasm + foundation_wasm_ui test suites green (133 tests incl. the
moved generator/validator/bundle suites and the APACHE_ARROW_JS embed
assertion); codegentools remaining suites green; zero clippy on both runtime
crates `--all-targets`; wasm32 check of both crates clean (stub bins);
`ewe_platform` binary checks green; both CLIs smoke-tested (`--help`, and the
error path emits via tracing ERROR).
