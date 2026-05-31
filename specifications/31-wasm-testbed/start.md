---
workspace_name: "ewe_platform"
spec_directory: "specifications/31-wasm-testbed"
this_file: "specifications/31-wasm-testbed/start.md"
created: 2026-05-31
---

# Start: WASM Testbed

## Agent Workflow

1. Read `requirements.md` (high-level overview + feature index)
2. **Identify language stack** from requirements.md "Language Stack" section - document languages clearly
3. **Read/generate language skills** - for each language in stack:
   - If skill exists: `.agents/skills/[language]-clean-code/skill.md` → read it
   - If skill missing: launch agent to generate one first, then read it
   - Add this item to the workflow list so future agents remember
4. Read `LEARNINGS.md` (past discoveries and mistakes)
5. Identify which feature you're working on from the Feature Index
6. Navigate to `features/[feature-name]/start.md` and follow that feature's workflow
7. **ALWAYS UPDATE LEARNINGS.md** after each completed task/milestone

---

**Workflow:** Requirements → Select Feature → Feature start.md → Follow feature workflow

## Dependencies

This spec depends on completed work from spec 28 (Cloudflare Workers Readiness):
- Feature 01-core-wasm-compat: Base wasm compatibility (ctrlc removal, feature flags, SSL switching)
- Feature 08-wasm-oauth-manager: wasm-bindgen OAuth manager patterns

Ensure these are complete before implementing the testbed.

---

_Created: 2026-05-31_
