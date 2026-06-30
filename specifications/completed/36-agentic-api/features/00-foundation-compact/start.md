---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/00-foundation-compact"
this_file: "specifications/36-agentic-api/features/00-foundation-compact/start.md"
created: 2026-06-14
---

# Start: foundation_compact (cross-platform substrate)

## Agent Workflow

1. Read `feature.md` (rename + vendor getrandom + vendor rand + fold foundation_rng; Open Decisions).
2. **Read the sources to vendor:** `/home/darkvoid/Boxxed/@formulas/src.rust/src.rust-random/getrandom`
   and `.../rand`. Read the existing crate `backends/foundation_webwasm/src/` (time), the crate to
   fold `backends/foundation_rng/`, and `backends/foundation_wasm/` (the wasm host runtime for web
   crypto). Read `foundation_core/src/valtron/executors/local.rs:2682` (confirms the executor yields
   to the JS loop — so NO sleep primitive here).
3. **Stack:** Rust (no_std-aware) + Cargo features + wasm. Read `.agents/skills/rust-clean-code/skill.md`.
   Follow `feedback_target_gate_native_tooling`.
4. Read `../../LEARNINGS.md`. Resolve OD-00-5..8 (rand scope, foundation_wasm crypto, licences,
   whether to delete 00a) before coding.
5. Generate `compacted.md`, clear, reload.
6. **One item at a time:** rename (workspace green) → vendor getrandom (`entropy/`) → vendor rand
   (`rng/`) → fold scru128 (`ids/`) → remove `foundation_rng`, repoint dependents → build matrix.
7. **Author the `fundamentals/` docs** (zero-to-expert) as part of completion.
8. Report; verify (native + wasm-bindgen + foundation-wasm); update `../../LEARNINGS.md`; move to 00b.

## Critical Notes

- **Vendor with upstream licences** (getrandom + rand are MIT/Apache-2.0) → `VENDORED.md`.
- **No external `rand`/`getrandom`** may remain in the workspace when done.
- **No `sleep` primitive** — backoff is valtron `TaskStatus::Delayed`/`Wait` (the executor yields to
  the JS event loop). `SleepIterator` has no place.

---

**Workflow:** feature.md → vendor sources + foundation_wasm + executor → Resolve OD-00 → Compact → rename green → vendor entropy → vendor rng → fold scru128 → repoint → build matrix → fundamentals → Report → 00b

---

_Created: 2026-06-14_
