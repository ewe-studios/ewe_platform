---
workspace_name: "ewe_platform"
spec_directory: "specifications/50-valtron-async-readiness"
this_file: "specifications/50-valtron-async-readiness/features/04-async-only-storage/start.md"
feature_name: "04-async-only-storage"
created: 2026-10-21
---

# Start: 04-async-only-storage

## Workflow

1. Read `feature.md` (this directory): §2 lists every caller, §7 the tasks in order.
2. Read `specifications/50-valtron-async-readiness/progress.md` and `learnings.md`.
3. Read `docs/bug-backlog.md` §11 (this feature) and §12–§15.
4. Read `backends/foundation_db/src/blocking.rs` and `src/core/storage_provider.rs`.
5. Read `.agents/skills/rust-valtron-usage/` (async all the way; sync only behind `blocking_boundary!`).
6. Work task by task (T1…T13): change one crate, run
   `python3 scripts/ci/ci.py run <crate> --step build --step clippy --step test --step wasm`,
   commit, push, update `progress.md`, `learnings.md` and the PR checklist.
7. Before deleting a trait (T10), grep the whole workspace, including `apps/` and `examples/`.

_Created: 2026-10-21_
