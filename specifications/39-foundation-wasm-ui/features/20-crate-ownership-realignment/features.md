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
├── src/build_tools/                 feature = "build-tools" (std, native-only)
│   ├── mod.rs                       WasmBinGenerator, WasmEntrypoint, plans
│   ├── error.rs / generator.rs / planner.rs / validator.rs
├── src/cli.rs                       feature = "cli" (clap): `wasm-bins` cmd
└── Cargo: build-tools = ["dep:foundation_codegen", "dep:toml", "dep:toml_edit"]
          cli = ["build-tools", "dep:clap"]

foundation_wasm_ui/                  (#![no_std] runtime — unchanged by default)
├── src/build_tools/                 feature = "build-tools"
│   ├── mod.rs                       WasmBundleGenerator (uses foundation_wasm::build_tools)
│   ├── js_wrapper.rs / bundler.rs
├── src/cli.rs                       feature = "cli": `wasm-bundle` cmd
└── embedded.rs                      + APACHE_ARROW_JS (embedded-js feature)
```

- The `build-tools`/`cli` features are **std features on no_std crates**:
  gated `extern crate std;`, OFF by default — wasm builds are untouched.
  Enabling them in a wasm32 build is a compile error by construction (std
  process/fs) — that's correct: build tooling never belongs in the artifact.
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
