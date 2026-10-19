---
workspace_name: "ewe_platform"
spec_directory: "specifications/65-agent-rpc-serving-system"
feature_directory: "specifications/65-agent-rpc-serving-system/features/05-cloudflare"
this_file: "specifications/65-agent-rpc-serving-system/features/05-cloudflare/start.md"
created: 2026-10-19
---

# Start: Cloudflare Deployment Feature

## Agent Workflow

1. Read `feature.md` (requirements + architecture + the "Changes required in
   existing crates" table + tasks).
2. Read its dependencies first: `../00-foundation/feature.md` (the wasm readiness
   this consumes) and `../01-agent-rpc/feature.md` (the model server being
   deployed). Note: the installer (`02-manifest-installer`) is **not** a
   dependency — Cloudflare deployment is not the installer's concern.
3. **Identify language stack** — Rust. Read `.agents/skills/rust-clean-code/skill.md`
   and `.agents/skills/rust-valtron-usage/skill.md` (Worker is single-threaded —
   no valtron threading there; native Sandboxes/Containers keep it).
4. Read `../../LEARNINGS.md` (past discoveries and mistakes).
5. Read `.agents/AGENTS.md` to identify your agent type, then your agent file in
   `.agents/agents/[agent-name].md`.
6. Read skills specified in your agent documentation.
7. **MANDATORY**: Generate `compacted.md` with all info using
   `.agents/skills/context-compaction/skill.md`.
8. Clear context, reload from `compacted.md` only, start work.
9. **Work on ONE item at a time** — one test, one function, one file.
10. Implement following TDD (test first, then code) — **one test at a time**.
11. Report to Main Agent when done (DO NOT commit).
12. Wait for verification to pass.
13. After commit: delete `compacted.md`, update `./PROGRESS.md`, move to next task.
14. **ALWAYS UPDATE ../../LEARNINGS.md** after each completed task/milestone.

**Start with T1 (thin orchestrator build target)** — it gates every Worker task.
(The `foundation_ai` wasm-clean gate already lives in `00-foundation` T1.)

---

**Workflow:** feature.md → dependencies (00, 01) → Rust skills → Learnings → AGENTS.md → Agent Doc → Skills → Compact → Clear → Reload → ONE ITEM AT A TIME → Implement → Report → Verify → Commit → Delete compacted.md → Next

---

_Created: 2026-10-19_
