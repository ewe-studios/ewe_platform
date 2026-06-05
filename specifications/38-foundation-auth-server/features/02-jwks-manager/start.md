---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/02-jwks-manager/start.md"
feature_name: "02-jwks-manager"
created: 2026-06-05
---

# Start: Feature 02 — JWKS Manager

## Workflow

1. Read `requirements.md` and `feature.md` in this directory
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_auth/src/shared/jwt.rs` (for `JwtVerifier` type)
4. Create `backends/foundation_auth/src/shared/jwks.rs`
5. Create `backends/foundation_auth/src/native/jwks.rs` (native HTTP fetch)
6. Create `backends/foundation_auth/src/wasm_bindgen/jwks.rs` (wasm fetch)
7. Update `shared/mod.rs` and `native/mod.rs` to include new modules
8. Run `cargo check 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
9. Add tests per `feature.md` Testing section
10. Update `LEARNINGS.md`

_Created: 2026-06-05_