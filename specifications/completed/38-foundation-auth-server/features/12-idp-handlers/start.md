---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/12-idp-handlers/start.md"
feature_name: "12-idp-handlers"
created: 2026-06-05
---

# Start: Feature 12 — IdP Handlers

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_http/src/shared/serve/mod.rs` (ServeWriter, respond helpers)
4. Read `backends/foundation_http/src/shared/context/mod.rs` (ContextBag)
5. Read all server services (Feature 11 output) and models (Feature 10 output)
6. Create `backends/foundation_auth/src/server/handlers/mod.rs`
7. Create each handler file:
   - `discovery.rs`, `authorize.rs`, `login.rs`, `mfa.rs`
   - `token.rs`, `userinfo.rs`, `jwks.rs`, `introspect.rs`, `device_authorize.rs`
8. Register handlers in `IdpServer::http_app()` (Feature 09)
9. Run `cargo check --features server 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
10. Add tests per `feature.md` Testing section
11. Update `LEARNINGS.md`

**Dependencies:** Requires Features 09 (IdpServer), 10 (Models), 11 (Services) to be completed first.

_Created: 2026-06-05_