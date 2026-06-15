---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/03-oidc-discovery/start.md"
feature_name: "03-oidc-discovery"
created: 2026-06-05
---

# Start: Feature 03 — OIDC Discovery Client

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_auth/src/shared/oauth.rs` (OAuthConfig)
4. Read `backends/foundation_auth/src/native/oauth.rs` (HTTP pattern)
5. Create `backends/foundation_auth/src/shared/discovery.rs`
6. Update `shared/mod.rs` to include discovery module
7. Extend `OAuthConfig` with `from_discovery()` or add `OidcDiscovery::to_oauth_config()`
8. Run `cargo check 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
9. Add tests per `feature.md` Testing section
10. Update `LEARNINGS.md`

_Created: 2026-06-05_