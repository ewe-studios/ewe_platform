# Learnings: Cloudflare Workers Readiness

## Completed Work

### 01-core-wasm-compat (2026-05-15)
- Removed dead `ctrlc` dependency from `foundation_core`
- Added `wasm` feature flag for `uuid/js`, `chrono/wasmbind`, `getrandom/js`
- SSL backend switching: `aws-lc-rs` (wasm) vs `ring` (native)

### 04-wire-restructure (2026-05-16)
- Split `foundation_core::wire` modules into shared (always compiled) and native (`#[cfg(not(wasm32))]`) submodules
- Deep client split in `simple_http`: moved pure logic to `client/shared/`, socket-dependent code to `client/native/`
- `event_source` and `websocket` shared types extracted, native consumers gated
- `http_stream` entirely native (uses `RawStream`)

## Lessons Learned

- **Duplicate numbering causes confusion**: Initially had two `03-` features (`03-http-wasm-compat` and `03-wire-restructure`). Fixed by renumbering sequentially.
- **Orphaned root-level modules**: When restructuring `foundation_http` into `shared/`/`native/`/`wasm/`, old root-level directories were left behind. Deleted them — they were already unreferenced by `lib.rs`.
- **Bridge not implemented**: `foundation_http/src/wasm/bridge/mod.rs` is a stub. The `web.rs` and `cf.rs` wasm-bindgen entry points were never written.

## Pending

- Feature 03-http-wasm-compat: ServeWriter trait, Server enum, wasm bridge
- Feature 05-db-wasm-compat: CF D1/R2/KV wasm-bindgen bridge
- Feature 06-example-app: Working login app
- Feature 07-ci-wasm-checks: CI wasm compilation checks
- Feature 08-wasm-oauth-manager: wasm-bindgen OAuth manager
- Feature 09-wasm-testbed: CLI-driven wasm test harness
