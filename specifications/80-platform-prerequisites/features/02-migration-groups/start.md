---
workspace_name: "ewe_platform"
spec_directory: "specifications/80-platform-prerequisites"
feature_directory: "specifications/80-platform-prerequisites/features/02-migration-groups"
this_file: "specifications/80-platform-prerequisites/features/02-migration-groups/start.md"
created: 2026-11-14
---

# Start: Migration Groups Feature

## Agent Workflow

1. Read `feature.md` (task T25).
2. Read `migrations.md` (the full design) and `../../resources/Migration Management.md`.
3. Read `../../LEARNINGS.md`.
4. Read `.agents/AGENTS.md`, `.agents/skills/rust-clean-code/skill.md` and
   `.agents/skills/rust-valtron-usage/skill.md`.
5. Check `foundation_db` for the existing migration list. Migration 023 is the known missing
   file; the new test must plan every file.
6. Work on one task at a time, test first, tests in `foundation_db/tests/`.
7. Report, verify, commit; update `../../progress.md` and `../../LEARNINGS.md`.

**Done when:** each database has a shared group and its own groups, each numbered from 1 with no
gaps, and `FleetRunner` applies the plan to many databases.

---

_Created: 2026-11-14_
