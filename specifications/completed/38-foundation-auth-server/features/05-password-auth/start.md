---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/05-password-auth/start.md"
feature_name: "05-password-auth"
created: 2026-06-05
---

# Start: Feature 05 — Password Authentication Flow

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_auth/src/native/oauth.rs` (HTTP pattern)
4. Read `backends/foundation_auth/src/wasm_bindgen/oauth.rs` (wasm HTTP pattern)
5. Read `backends/foundation_auth/src/shared/types.rs` (AuthCredential types)
6. Create `backends/foundation_auth/src/native/password_auth.rs`
7. Create `backends/foundation_auth/src/wasm_bindgen/password_auth.rs` (gated behind `wasm-bindgen-oauth`)
8. Update `native/mod.rs` and `wasm_bindgen/mod.rs`
9. Run `cargo check 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
10. Add tests per `feature.md` Testing section
11. Update `LEARNINGS.md`

_Created: 2026-06-05_