# Feature 20: Crate Ownership Realignment + apache-arrow.js Embedding

**Origin:** post-F10 review (2026-06-12) — build tooling should live with the
crates whose artifacts it builds, and the bundled `apache-arrow.js` (already
shipped in `foundation_wasm_ui/runtimes/`) should be embeddable like the
other runtime assets.

## 1. The ownership problem

F10 placed `wasm_bins` (entrypoint discovery + `bin/` generation) and
`wasm_bundle` (JS wrappers + single-file bundling + CLI) in
`foundation_codegentools`. But:

- `wasm_bins` generates and compiles **foundation_wasm** binaries — the ABI
  crate is the natural owner of "how do my entrypoints become wasm binaries".
- `wasm_bundle` generates **foundation_wasm_ui** deliverables (JS wrappers
  over the owned runtimes, bundles, runtime-asset copies) — the UI crate is
  the natural owner of "how does my runtime ship".
- Each crate should OWN ITS CLI: versioning, defaults, and templates evolve
  with the runtime they wrap, not with a generic tools crate.

`foundation_codegentools` keeps what is genuinely generic: the crate scanner
consumers, schema generation, and the wasm FILE inspection CLI (wasmbin port).

## 2. Target layout

```
foundation_wasm/                     (#![no_std] runtime — unchanged by default)
├── src/build_tools/                 cfg(not(target_arch = "wasm32")) (std, native-only)
│   ├── mod.rs                       WasmBinGenerator, WasmEntrypoint, plans
│   ├── error.rs / generator.rs / planner.rs / validator.rs
├── src/cli.rs                       `wasm-bins` cmd (clap)
├── src/bin/ewe_wasm_bins.rs         standalone CLI (stub main on wasm32)
└── Cargo: [target.'cfg(not(target_arch = "wasm32"))'.dependencies]
          foundation_codegen, toml, toml_edit, clap, tracing(+subscriber)

foundation_wasm_ui/                  (#![no_std] runtime — unchanged on wasm)
├── src/build_tools/                 cfg(not(target_arch = "wasm32"))
│   ├── mod.rs                       WasmBundleGenerator (uses foundation_wasm::build_tools)
│   ├── js_wrapper.rs / bundler.rs
├── src/cli.rs                       `wasm-bundle` cmd
├── src/bin/ewe_wasm_bundle.rs       standalone CLI (stub main on wasm32)
└── embedded.rs                      + APACHE_ARROW_JS (embedded-js feature)
```

- Build tooling is **target-gated, not feature-gated** (review decision: the
  CLI should build every time): deps under
  `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`, modules behind
  `#[cfg(not(target_arch = "wasm32"))]` with a gated `extern crate std;`.
  Native builds always get the tooling; wasm32 builds never see it — build
  tooling never belongs in the artifact.
- Logs go through `tracing` (`error!`/`info!`; fmt subscriber installed by
  the standalone bins) — never `eprintln!`. The `list`/`plan` dry-run
  reports are command OUTPUT and stay on stdout.
- No dependency cycles: `foundation_codegen` does not depend on
  `foundation_wasm`; `foundation_wasm_ui` already depends on
  `foundation_wasm`.
- `bin/platform` re-wires its `wasm_bins` subcommand to
  `foundation_wasm::cli` and gains `wasm-bundle` from
  `foundation_wasm_ui::cli`.

## 3. apache-arrow.js embedding

`runtimes/apache-arrow.js` (the bundled Apache Arrow JS library, G3/G25 —
needed by server-content-type consumers reading wire VERSION 2 / real Arrow
IPC) joins `embedded.rs` as `APACHE_ARROW_JS` under the existing
`embedded-js` feature, alongside `FOUNDATION_WASM_UI_JS` — one flag embeds
the whole serving set.

## 4. Testing

- Moved tests move with their code (`wasm_bins` unit tests →
  `foundation_wasm/tests`; bundle tests → `foundation_wasm_ui/tests`),
  re-pointed at the new paths, green under the new features.
- `embedded.rs` test asserts `APACHE_ARROW_JS` carries the Arrow library
  marker alongside the existing runtime assertions.
- `foundation_codegentools` still builds + its remaining suites stay green.
- Default-feature builds of both runtime crates remain no_std-clean
  (workspace check green).
