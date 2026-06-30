---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/27-foundation-vectors-code-graph"
this_file: "specifications/36-agentic-api/features/27-foundation-vectors-code-graph/start.md"
created: 2026-06-14
---

# Start: foundation_vectors code-graph

## Agent Workflow

1. Read `feature.md` (the graphify replication blueprint) + Decision 07/14.
2. **Read the graphify source directly** — `@formulas/src.rust/src.AI/src.Graphify/graphify/graphify/`:
   `extract.py` (AST core, 3611 lines), `build.py`, `cluster.py`, `analyze.py`, `llm.py`,
   `detect.py`, `__main__.py`, `skill.md` (subagent prompts). The feature cites exact line refs.
3. **Stack:** Rust + tree-sitter. Read `.agents/skills/rust-clean-code/skill.md`. Confirm F24 landed.
4. Read `../../LEARNINGS.md`. Resolve OD-27-1..7 (esp. the 11a/b/c split + language scope) before coding.
5. Generate `compacted.md`, clear, reload.
6. **One item at a time:** LanguageConfig + walk → per-language configs (priority langs) → call
   resolution → rationale → build/dedup → cluster → query. Fixture-test each language.
7. Report; verify (native build + wasm query path); update `../../LEARNINGS.md`; move to Feature 28.

---

**Workflow:** feature.md → graphify SOURCE → tree-sitter + Rust skill → Resolve OD-27 → Compact → ONE LANG AT A TIME (fixtures) → Report → Verify → 12

---

_Created: 2026-06-14_
