---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/16-walrus-transform-port/start.md"
feature_name: "16-walrus-transform-port"
created: 2026-06-10
status: deferred
---

# Start: Feature 16 — Port walrus transformation capabilities (DEFERRED)

## Status: DEFERRED — recorded intention

We reviewed `walrus` (https://github.com/rustwasm/walrus, MIT/Apache-2.0, © Nick Fitzgerald,
v0.23.3, source at `/home/darkvoid/Boxxed/@formulas/src.rust/src.wasm/src.wasmbindgen/walrus/`)
for porting its transformation capabilities into `foundation_codegen`. It is **large and
dependency-heavy** (~12.5k LOC; depends on `wasmparser`, `wasm-encoder`, `id-arena`, `gimli`,
`rayon`, `walrus-macro`) and overlaps the owned wasmbin port (F15) for parse/serialize.

**Decision: defer the full port.** Its unique value (a mutable IR with automatic reference
bookkeeping for heavy structural rewrites) is not needed yet, and the one capability we actually
want — reference auto-cleanup on deletion — is cheaper to add onto the wasmbin port (F15). This
feature exists to record the intent, the assessment, and the trigger to revisit.

## Workflow (when un-deferred)

1. Read `features.md` — the complexity assessment and what we'd port.
2. Read decision 031 (own it; wasm-format tools allowed) and F15 (wasmbin — the owned default).
3. Study the walrus source: `ir/`, `module/functions/local_function/{mod,emit,context}.rs`,
   `function_builder.rs`, `tombstone_arena.rs`, `passes/{used,gc}.rs`, `module/debug/` (DWARF).
4. Re-evaluate: cherry-pick onto wasmbin vs. full port.

_Created: 2026-06-10_
