# 01-thread-ctx — Feature Start

1. Read `feature.md`.
2. Read `../LEARNINGS.md` (parent spec) and parent `VERIFICATION.md` if present.
3. Read `.agents/AGENTS.md`.
4. Read `.agents/skills/rust-valtron-usage/skill.md` and `rust-valtron-iterator/skill.md`.
5. Generate `compacted.md` from the feature context.
6. Clear context and reload.
7. Work ONE task at a time.
8. TDD: write the test first, then the implementation.
9. Report to Main Agent.
10. Wait for verification.
11. Delete `compacted.md` after commit.

Build on `00-ctx`. The churn is compiler-driven: add `ctx: Ctx` to the two traits,
then follow the compile errors outward (wrappers → core impls → backends). Confirm
the RNG open decision from `00-ctx` before wiring `ctx.rng()` into any task.
