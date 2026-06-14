---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/14-vectorstore-cloudflare-external"
this_file: "specifications/36-agentic-api/features/14-vectorstore-cloudflare-external/start.md"
created: 2026-06-14
---

# Start: VectorStore Cloudflare + external

## Agent Workflow

1. Read `feature.md` + Decision 07/03b. Confirm F12 (`AsyncVectorStore`) landed.
2. **Stack:** Rust + wasm/CF + HTTP. Read `.agents/skills/rust-clean-code/skill.md`. Reuse F06's
   D1/KV bindings; study the Chroma source (`@formulas/.../src.VectorDB/src.Chroma`).
3. Research CF Vectorize availability + provider REST APIs first.
4. Read `../../LEARNINGS.md`. Resolve OD-14-1..5 (incl. 14a/14b split).
5. Generate `compacted.md`, clear, reload.
6. **One backend at a time:** CF D1/KV → CF Vectorize → Chroma → others. Mock tests each.
7. **Author `fundamentals/` docs.**
8. Report; verify; update `../../LEARNINGS.md`; move to Feature 15.

---

**Workflow:** feature.md → research → F06 bindings + Chroma → Resolve OD-14 → Compact → ONE BACKEND AT A TIME → fundamentals → Report → Verify → 15

---

_Created: 2026-06-14_
