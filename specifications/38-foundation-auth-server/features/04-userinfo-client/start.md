---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/04-userinfo-client/start.md"
feature_name: "04-userinfo-client"
created: 2026-06-05
---

# Start: Feature 04 — UserInfo Client

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Create `backends/foundation_auth/src/shared/userinfo.rs`
4. Update `shared/mod.rs` to include userinfo module
5. Run `cargo check 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
6. Add tests per `feature.md` Testing section
7. Update `LEARNINGS.md`

_Created: 2026-06-05_