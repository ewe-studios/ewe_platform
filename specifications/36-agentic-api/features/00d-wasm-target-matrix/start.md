---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/00d-wasm-target-matrix"
this_file: "specifications/36-agentic-api/features/00d-wasm-target-matrix/start.md"
created: 2026-06-15
---

# Start: wasm target matrix

## Agent Workflow

1. Read `feature.md` (the 4-target matrix + cfg discipline + per-crate prep).
2. **Read the two crates:** `backends/foundation_wasm/` (Cargo.toml features `web`/`embedded-js`;
   `src/host_runtime.rs`, `frames.rs`, `intervals.rs`, `schedule.rs` — note they gate on `target_arch`
   not `target_os`), and `backends/foundation_testbed/` (Cargo.toml `wasm` feature; `src/wasm/` — the
   unknown-unknown browser/Deno/CF harness). Read `foundation_core/.../local.rs:2682` (JS-loop yield).
3. **Stack:** Rust + wasm targets + WASI/emscripten. Read `.agents/skills/rust-clean-code/skill.md`.
   Confirm F00 (foundation_compact entropy/time across the matrix) landed.
4. Read `../../LEARNINGS.md`. Resolve OD-00d-1..5 (host scope, runtime choice, cfg refactor) first.
5. Generate `compacted.md`, clear, reload.
6. **One item at a time:** cfg discriminators + audit → foundation_wasm WASI/emscripten host →
   foundation_testbed runners → per-target build checks.
7. **Author the `fundamentals/` docs.**
8. Report; verify (build each target); update `../../LEARNINGS.md`.

## Critical Notes

- **target_os-aware, not blanket `target_arch="wasm32"`** — emscripten/WASI have far more std than
  unknown-unknown. Native-C tooling (fff/llama) → native+emscripten; pure Rust → all four.
- `mise.toml` already adds `wasm32-wasip1`; EMSDK is vendored at `tools/emsdk`.

---

**Workflow:** feature.md → foundation_wasm + foundation_testbed + executor → Resolve OD-00d → Compact → ONE ITEM → fundamentals → Report → Verify

---

_Created: 2026-06-15_
