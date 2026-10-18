# 00-cleanup — Feature Start

1. Read `feature.md` (the cross-crate audit is in there — read it first).
2. Read `../LEARNINGS.md` (parent spec) and parent `VERIFICATION.md` if present.
3. Read `.agents/AGENTS.md`.
4. Read `.agents/skills/rust-valtron-usage/skill.md` and `rust-valtron-iterator/skill.md`.
5. Generate `compacted.md` from the feature context.
6. Clear context and reload.
7. Work ONE task at a time (task order = the audit's removal order).
8. TDD: the removal is a deletion; the existing combinator tests in
   `stream_iterators.rs` are the regression net — run them after each deletion.
9. Report to Main Agent.
10. Wait for verification.
11. Delete `compacted.md` after commit.

The audit in `feature.md` is authoritative — do not remove the combinator layer
(`extensions/{streams,tasks}/*`) or `RecvIterator`; only the two dead types.
