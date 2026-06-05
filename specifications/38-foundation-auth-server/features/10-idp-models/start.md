---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/10-idp-models/start.md"
feature_name: "10-idp-models"
created: 2026-06-05
---

# Start: Feature 10 — IdP Models

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_db/src/core/schema/sql/002_create_users.sql` (user table)
4. Read `backends/foundation_auth/src/shared/two_factor.rs` (constant_time_eq utility)
5. Create `backends/foundation_auth/src/server/models/mod.rs`
6. Create `backends/foundation_auth/src/server/models/user.rs`
7. Create `backends/foundation_auth/src/server/models/client.rs`
8. Create `backends/foundation_auth/src/server/models/code.rs`
9. Create `backends/foundation_auth/src/server/models/token.rs`
10. Run `cargo check --features server 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
11. Add tests per `feature.md` Testing section
12. Update `LEARNINGS.md`

_Created: 2026-06-05_