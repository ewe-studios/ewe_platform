---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/14-cedar-policy-engine/start.md"
feature_name: "14-cedar-policy-engine"
created: 2026-06-05
---

# Start: Feature 14 — Cedar Policy Engine

## Workflow

This feature creates a **new crate** (`backends/foundation_cedar/`), not modifications to an existing one. The implementation should follow the native/shared/wasm split pattern established in `foundation_db`.

### Phase 1: Crate Setup + Core Engine

1. Read `requirements.md` and `feature.md` in this directory
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_db/src/` for the native/shared/wasm split pattern
4. Read valtron skills: `.agents/skills/rust-valtron-usage/skill.md`, `.agents/skills/rust-valtron-iterator/skill.md`
5. Read Cedar reference sources:
   - `/home/darkvoid/Boxxed/@formulas/src.rust/src.auth/src.CedarPolicy/cedar/cedar-policy/src/api.rs`
   - `/home/darkvoid/Boxxed/@formulas/src.rust/src.auth/src.CedarPolicy/cedar-local-agent/src/public/mod.rs`
   - `/home/darkvoid/Boxxed/@formulas/src.rust/src.auth/src.CedarPolicy/cedar-local-agent/src/public/simple.rs`
   - `/home/darkvoid/Boxxed/@formulas/src.rust/src.gedweb/cf-connectrpc-middleware/crates/connectrpc-cedar/src/authorizer.rs`
   - `/home/darkvoid/Boxxed/@formulas/src.rust/src.auth/src.CedarPolicy/cedar-examples/cedar-rust-hello-world/src/main.rs`
6. Create `backends/foundation_cedar/Cargo.toml` with feature flags
7. Create `backends/foundation_cedar/src/lib.rs` with crate root and re-exports
8. Create `backends/foundation_cedar/src/core/` module:
   - `engine.rs` — `CedarEngine` (schema + policies + authorizer)
   - `request.rs` — `CedarRequest` builder
   - `response.rs` — `CedarResponse` wrapper
   - `errors.rs` — `CedarError` enum
   - `policy/policy_set.rs` — `PolicySetSource`
   - `policy/entity_provider.rs` — `EntityProvider` + `SyncEntityProvider` traits
9. Run `cargo check -p foundation_cedar 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
10. Fix issues as they appear
11. Add tests per `feature.md` Testing section

### Phase 2: PolicyStore Traits + Native Backends

1. Create `backends/foundation_cedar/src/storage/` module:
   - `traits.rs` — `PolicyStore` (sync) + `AsyncPolicyStore` traits
   - `shared/parser.rs` — parse `.cedar` files and JSON policies
   - `shared/resolver.rs` — raw text → `PolicySet` resolution
2. Create native backends:
   - `storage/native/local_file.rs` — read from directory
   - `storage/native/r2_store.rs` — Cloudflare R2 (via foundation_db)
   - `storage/native/d1_store.rs` — D1/Turso SQLite (via foundation_db)
3. Run `cargo check -p foundation_cedar --features native 2>&1 | tee /tmp/cargo-check.log`
4. Add tests for each backend

### Phase 3: Wasm Backends + Git Integration

1. Create wasm backends:
   - `storage/wasm_bindgen/d1_wasm.rs` — D1 via wasm-bindgen
   - `storage/wasm_bindgen/r2_wasm.rs` — R2 via wasm-bindgen
   - `storage/wasm_bindgen/git_wasm.rs` — git via HTTP fetch / gix-odb
2. Create git backend (native):
   - `storage/native/git_store.rs` — uses `gix` for clone/fetch/read
3. Run `cargo check -p foundation_cedar --target wasm32-unknown-unknown --features wasm 2>&1 | tee /tmp/cargo-check.log`
4. Add tests

### Phase 4: Middleware + Integration

1. Create `backends/foundation_cedar/src/middleware/` module:
   - `tower_layer.rs` — tower middleware for HTTP authorization
   - `extractors.rs` — request → Cedar principal/action/resource extractors
2. Integrate with IdP server (feature 09-12):
   - Add `cedar` feature to `foundation_auth` Cargo.toml
   - Add `foundation_cedar` dependency to `foundation_auth`
   - Create integration point in `IdpServer`
3. Run `cargo check -p foundation_auth --features server,cedar 2>&1 | tee /tmp/cargo-check.log`

### Phase 5: Fundamentals Documentation

1. Create `backends/foundation_cedar/fundamentals/` directory:
   - `cedar_basics.md` — Cedar language primer
   - `cedar_patterns.md` — RBAC, ABAC, ReBAC patterns
   - `cedar_wasm.md` — WASM / CF Workers guide
   - `cedar_partial_eval.md` — partial evaluation guide
   - `cedar_integration_guide.md` — integration with other crates
2. Reference the existing Cedar examples and connectrpc-cedar code for real-world patterns

### Phase 6: Migration (if D1 backend used)

1. Add migration for `cedar_policies` table to `foundation_db` schema
2. Update migration count and tests

### Final Steps

1. Run full test suite: `cargo test -p foundation_cedar --all-features 2>&1 | tee /tmp/cargo-test.log`
2. Run `cargo check -p foundation_auth --features server,cedar 2>&1 | tee /tmp/cargo-check.log`
3. Update `LEARNINGS.md`
4. Update `requirements.md` feature index status

**Dependencies:** This feature is independent — no features from spec 38 need to be completed first. However, integration with the IdP server (feature 09-12) requires those features to exist.

_Created: 2026-06-05_
