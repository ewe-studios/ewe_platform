---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/07-nonce-support/start.md"
feature_name: "07-nonce-support"
created: 2026-06-05
---

# Start: Feature 07 — Nonce Support

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_auth/src/shared/oauth.rs` (current OAuthConfig, OAuthManager)
4. Read `backends/foundation_auth/src/shared/jwt.rs` (VerifiedClaims after Feature 01)
5. Read `backends/foundation_auth/src/shared/credential_store.rs` (OAuthState pattern)
6. Extend `OAuthConfig` with `nonce` field
7. Extend `OAuthConfigBuilder` with `nonce()` method
8. Extend `OAuthManager::get_authorization_url()` to include nonce
9. Add `OAuthManager::generate_nonce()`
10. Add `VerifiedClaims::verify_nonce()` (after Feature 01)
11. Run `cargo check 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
12. Add tests per `feature.md` Testing section
13. Update `LEARNINGS.md`

_Created: 2026-06-05_