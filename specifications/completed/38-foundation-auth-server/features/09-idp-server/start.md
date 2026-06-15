---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/09-idp-server/start.md"
feature_name: "09-idp-server"
created: 2026-06-05
---

# Start: Feature 09 — IdP Server

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_http/src/shared/app/mod.rs` (HttpApp builder pattern)
4. Read `backends/foundation_http/src/shared/serve/mod.rs` (ServeWriter, respond helpers)
5. Read `backends/foundation_http/src/shared/middleware/mod.rs` (RateLimiter, CorsMiddleware)
6. Update `backends/foundation_auth/Cargo.toml` — add foundation_http dep, server feature
7. Create `backends/foundation_auth/src/server/mod.rs`
8. Create `backends/foundation_auth/src/server/config.rs` (IdpConfig, PasswordPolicy)
9. Create `backends/foundation_auth/src/server/idp_server.rs`
10. Update `backends/foundation_auth/src/lib.rs` — add `pub mod server;` (cfg-gated)
11. Run `cargo check --features server 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
12. Add tests per `feature.md` Testing section
13. Update `LEARNINGS.md`

_Created: 2026-06-05_