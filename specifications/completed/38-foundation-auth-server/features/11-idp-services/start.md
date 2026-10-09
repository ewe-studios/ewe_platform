---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/11-idp-services/start.md"
feature_name: "11-idp-services"
created: 2026-06-05
---

# Start: Feature 11 — IdP Services

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_auth/src/shared/session.rs` (SessionManager)
4. Read `backends/foundation_auth/src/shared/jwt.rs` (JwtSigningKey after Feature 01)
5. Read `backends/foundation_db/src/core/storage_provider.rs` (QueryStore, DataValue, SqlRow)
6. Create `backends/foundation_auth/src/server/services/mod.rs`
7. Create `backends/foundation_auth/src/server/services/token_service.rs`
8. Create `backends/foundation_auth/src/server/services/user_service.rs`
9. Create `backends/foundation_auth/src/server/services/client_service.rs`
10. Create `backends/foundation_auth/src/server/services/session_service.rs`
11. Run `cargo check --features server 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
12. Add tests per `feature.md` Testing section
13. Update `LEARNINGS.md`

_Created: 2026-06-05_