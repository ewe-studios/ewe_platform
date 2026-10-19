# 06-state-task — Feature Start

1. Read `feature.md`.
2. Read `../../LEARNINGS.md` (parent spec) and parent `VERIFICATION.md` if present.
3. Read `.agents/AGENTS.md`.
4. Read `.agents/skills/rust-valtron-usage/skill.md` and `rust-valtron-iterator/skill.md`.
5. Generate `compacted.md` from the feature context.
6. Clear context and reload.
7. Work ONE task at a time.
8. TDD: write the test first, then the implementation.
9. Report to Main Agent.
10. Wait for verification.
11. Delete `compacted.md` after commit.

Build on `05-thread-ctx` (the `ctx: Ctx` signature is already in place). Add
`StateTask` first, then `TransitionPending` + `map_pending`, then `AsyncStateTask`
(reuse `FutureTask`'s wake_queue seam), then `#[valtron::task]` (reference
`foundation_macros/src/valtron_entry.rs` and `crate_paths.rs`).
