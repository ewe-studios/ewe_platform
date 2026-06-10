# `foundation_codegen::wasm` — type-safe WebAssembly binary model

A 1:1 typed model of the WebAssembly binary format: parse, edit, and re-serialize
`.wasm` with **minimal diffs** — `Lazy<T>` keeps undecoded parts as raw bytes and
re-emits them verbatim, so untouched sections round-trip byte-identically.

Ported & adapted from [**wasmbin**](https://github.com/RReverser/wasmbin)
(upstream **v0.9.2**), © Google Inc. / Ingvar Stepanyan, licensed Apache-2.0 —
see [`vendor/LICENSE.wasmbin`](vendor/LICENSE.wasmbin), the workspace `NOTICE`,
and the author's write-up:
<https://rreverser.com/wasmbin-yet-another-webassembly-parser-serializer/>.

## Modifications versus upstream (Apache-2.0 §4(b))

- Lives as a **module of `foundation_codegen`** (`crate::wasm::…` paths) instead of
  a standalone crate; decision 031 (owned WASM infrastructure), feature 15.
- The derive macros come from **`foundation_macros`** (`Wasmbin`,
  `WasmbinCountable`, `Visit`) and resolve trait paths via `proc-macro-crate`, so
  they also work outside `foundation_codegen`.
- `io::PathItem` and `DecodeError::in_path` are **public** (the derives can expand
  in downstream crates; upstream kept them `pub(crate)`).
- Upstream's optional `serde` feature is renamed **`wasm-serde`** (this crate has a
  non-optional `serde` dependency, and cargo forbids a feature sharing its name).
- One let-chain rewritten for edition-2021 compatibility (`builtins/lazy.rs`).
- Planned extension: **WAT (text) ⇄ binary** conversion (feature 15 — net-new, not
  from upstream).

## Quick use

```rust
use foundation_codegen::wasm::{Module, sections::payload};

let bytes = std::fs::read("module.wasm")?;
let mut module = Module::decode_from(bytes.as_slice())?;

// Type-safe read: enumerate function exports.
if let Some(exports) = module.find_std_section::<payload::Export>() {
    for export in exports.try_contents()? {
        println!("{} -> {:?}", export.name, export.desc);
    }
}

// Minimal-diff edit: untouched sections are copied verbatim on encode.
let out = module.encode_into(Vec::new())?;
```

Validated by `tests/wasm_roundtrip_tests.rs`: byte-identical round-trips across all
real `.wasm` fixtures in this workspace (e2e modules + 21 legacy integration
modules), export-section enumeration, and a custom-section injection edit.

Wasm proposals ride the same upstream feature gates: `extended-name-section`,
`threads`, `custom-page-sizes` (umbrella: `proposals`).
