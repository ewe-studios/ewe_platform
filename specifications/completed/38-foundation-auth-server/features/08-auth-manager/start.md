---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/08-auth-manager/start.md"
feature_name: "08-auth-manager"
created: 2026-06-05
---

# Start: Feature 08 — Auth Manager

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read all existing components:
   - `backends/foundation_auth/src/shared/jwt.rs` (JwtManager)
   - `backends/foundation_auth/src/shared/session.rs` (SessionManager)
   - `backends/foundation_auth/src/shared/auth_state.rs` (AuthStateMachine)
   - `backends/foundation_auth/src/shared/credential_store.rs` (CredentialStorage)
   - `backends/foundation_auth/src/shared/types.rs` (AuthCredential, Authenticated)
4. Create `backends/foundation_auth/src/shared/auth_manager.rs`
5. Update `shared/mod.rs` and `lib.rs` re-exports
6. Run `cargo check 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
7. Add tests per `feature.md` Testing section
8. Update `LEARNINGS.md`

_Created: 2026-06-05_