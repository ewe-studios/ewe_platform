---
workspace_name: "ewe_platform"
spec_directory: "specifications/80-platform-prerequisites"
feature_directory: "specifications/80-platform-prerequisites/features/00-env-and-side-effects"
this_file: "specifications/80-platform-prerequisites/features/00-env-and-side-effects/start.md"
created: 2026-11-14
---

# Start: Env and Side Effects Feature

## Agent Workflow

1. Read `feature.md` (tasks T1, T2 and T12; the story and acceptance scenarios).
2. Read `env.md` (the full design) and `deterministic-simulation-testing.md`.
3. Read `../../../76-shared-context/requirements.md` and `progress.md` (the baseline).
4. Read `../../LEARNINGS.md`.
5. Read `.agents/AGENTS.md`, `.agents/skills/rust-clean-code/skill.md` and
   `.agents/skills/rust-valtron-usage/skill.md`.
6. **Baseline first:** verify what spec 76 landed on master. Record each already-done item with
   its commit; do not write a second context design.
7. Work on one task at a time. Write the test first, in `tests/`. No inline `#[cfg(test)]` module.
8. Report to the Main Agent. After the commit, update `../../progress.md` and
   `../../LEARNINGS.md`.

**Done when:** every side effect in the feature reads `Env` from `Ctx`, a missing `Env` is a
typed error, and a DST run replays from one seed.

---

_Created: 2026-11-14_
