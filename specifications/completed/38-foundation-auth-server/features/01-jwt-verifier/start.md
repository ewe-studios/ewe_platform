---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/01-jwt-verifier/start.md"
feature_name: "01-jwt-verifier"
created: 2026-06-05
---

# Start: Feature 01 — JWT Verifier

## Workflow

1. Read `requirements.md` at `specifications/38-foundation-auth-server/requirements.md`
2. Read `feature.md` in this directory
3. Read `.agents/skills/rust-clean-code/skill.md`
4. Read existing `backends/foundation_auth/src/shared/jwt.rs`
5. Implement `JwtVerifier`, `JwtVerifierConfig`, `VerifiedClaims`, `JwtSigningKey`
6. Add new `JwtError` variants
7. Deprecate `JwtToken::from_token()`, add `from_verified_token()`
8. Run `cargo check 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
9. Fix issues as they appear
10. Run existing tests, ensure no regressions
11. Add new tests per `feature.md` Testing section
12. Update `LEARNINGS.md`

_Created: 2026-06-05_