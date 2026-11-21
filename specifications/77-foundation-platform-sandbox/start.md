---
workspace_name: "ewe_platform"
spec_directory: "specifications/77-foundation-platform-sandbox"
this_file: "specifications/77-foundation-platform-sandbox/start.md"
created: 2026-11-14
---

# Start: Foundation Platform Sandbox

## Read first

- `requirements.md` (mission, laws, the handle doctrine in section 6.0, the feature graph and
  success criteria).
- `progress.md` and `learnings.md`, especially section 10 (the handle doctrine, the one-owner
  rule for shared blocks, and the renames of 2026-11-15).
- The four specifications this one builds on: `../84-context-injection/requirements.md` (the
  `Env` and the `IdSource`), `../85-wasm-fetch-transport/requirements.md` (the fetch plumbing),
  `../86-auth-primitives/requirements.md` (`Secret` and the bearer parser) and
  `../87-mutual-tls/requirements.md` (the CA, the trust store and mTLS).
- `.agents/AGENTS.md`, `.agents/skills/rust-clean-code/skill.md`,
  `.agents/skills/rust-valtron-usage/skill.md`,
  `.agents/skills/rust-valtron-iterator/skill.md` and
  `.agents/skills/test-driven-development/skill.md`.

## Agent Workflow

1. Generate `compacted.md` with `.agents/skills/context-compaction/skill.md`.
2. Pick a feature from the table in section 7 of `requirements.md`.
3. Navigate to `features/[feature]/start.md`.
4. Respect the order below.
5. **ALWAYS UPDATE `learnings.md`** after each completed task or milestone.

**Order:** 00 first. Then 01, 02, 03 and 04. Then 05, then 06 (MVP-A). Feature 07 starts after
01 and 02. Then 08 (MVP-B). Then 09, 10 and 17. Then 11, 12, 13, 14 and 15. Feature 16 is a
draft and is not scheduled.

**Ownership:** this spec owns Cloudflare container control (feature 07 section 6.4.1, feature
00 section 6.13) and Cloudflare deployment (feature 08 section 6.8). Spec 83 feature 06 builds
the agent-rpc Worker on those surfaces. Do not move them to spec 83.

---

_Created: 2026-11-14_
