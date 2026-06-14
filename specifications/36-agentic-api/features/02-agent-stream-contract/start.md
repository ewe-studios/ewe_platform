---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/02-agent-stream-contract"
this_file: "specifications/36-agentic-api/features/02-agent-stream-contract/start.md"
created: 2026-06-14
---

# Start: Agent Stream & Progress Contract

## Agent Workflow

1. Read `feature.md` + Decision 11 (`../../decisions/11-agentic-loop-architecture.md`, esp. TODO #9).
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` and
   `rust-valtron-iterator/skill.md`. Read the model stream `StreamIterator<D=Messages, P=ModelState>`
   in `types/mod.rs` and `foundation_core/src/valtron/streams.rs`.
3. Read `../../LEARNINGS.md`. Confirm F01 (`SessionRecord`) is available.
4. Resolve OD-02-1 (SessionRecord vs Messages) before coding — it shapes the public type.
5. Generate `compacted.md`, clear, reload.
6. **One item at a time:** `AgentProgress` enum + docs → lift mapping → tests.
7. Report; verify; update `../../LEARNINGS.md`; move to Feature 03.

---

**Workflow:** feature.md → Decision 11 → valtron skills → Resolve OD-02-1 → Compact → ONE ITEM → Report → Verify → 03

---

_Created: 2026-06-14_
