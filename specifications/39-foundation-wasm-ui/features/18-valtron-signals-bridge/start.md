---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/18-valtron-signals-bridge/start.md"
feature_name: "18-valtron-signals-bridge"
created: 2026-06-12
---

# Start: Feature 18 — Valtron Signals Bridge

**Scheduled LAST** (after F03-F11). The API design in `features.md` is awaiting
user review — resolve the §6 review questions before implementing.

## Workflow

1. Read `features.md` (this feature) — the full API design
2. Read `features/02-signal-system/status.md` Q&A (RefCell-vs-Mutex rationale — the origin of this feature)
3. Read `.agents/skills/rust-clean-code/skill.md`
4. Read `backends/foundation_core/src/valtron/README.md` (queues, TaskIterator, the `#[valtron]`/`#[valtron_test]` macros)
5. Confirm the §6 review answers with the user
6. Implement behind `feature = "valtron"` per `features.md`
7. Tests per §5 (use `#[valtron_test]` for the multi-pool e2e)
8. Update `LEARNINGS.md` + `status.md` + Feature Index

_Created: 2026-06-12_
