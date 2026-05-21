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

### --all-features Compilation (2026-05-20)

Three categories of `--all-features` breakage were identified and fixed:

**1. Mutually-exclusive TLS feature guards** (`foundation_core`)

`Connection::Tls` and related types used `cfg(all(feature="ssl-X", not("ssl-Y"), not("ssl-Z")))` guards. With `--all-features`, all three SSL backends (`ssl-rustls`, `ssl-openssl`, `ssl-native-tls`) are enabled simultaneously, so NONE of the mutually-exclusive guards matched — the `Tls` variant vanished entirely, causing 15 "not found" errors.

**Fix**: Replaced mutual-exclusion with priority-based resolution: `ssl-rustls` wins, then `ssl-openssl`, then `ssl-native-tls`. Applied consistently across:
- TLS type alias in `connection/mod.rs`
- `Connection::Tls` enum variant
- All `From<TcpStream>` / `From<TlsStream>` impls
- Module exports in `ssl/mod.rs`
- `initialize_tls_provider()` in `ssl/rustls.rs` (aws-lc-rs over ring)

The same pattern was already used in `ssl/mod.rs` for conditional module inclusion — we extended it to type resolution.

**2. Unstable features not at crate root** (`foundation_core`)

`#![feature(unix_socket_peek, tcp_linger)]` was placed inside a submodule, but Rust requires feature attributes at the crate root (`lib.rs`).

**Fix**: Moved to crate root with feature gate: `#![cfg_attr(feature = "nightly", feature(unix_socket_peek, tcp_linger))]`. Added `socket2` as optional dependency behind `nightly = ["socket2"]`. The `set_reuse_address`/`reuse_address` methods are gated behind `#[cfg(feature = "nightly")]`.

**3. Platform-specific dependencies causing cross-platform compile_error** (`foundation_ai`)

`candle-metal` feature → `candle-core/metal` → `objc2` crate, which has `compile_error!("objc2 only works on Apple platforms")`. With `--all-features` on Linux, this caused an immediate build failure.

**Fix**: Replaced the feature-flag approach with Cargo's target-specific dependencies:
```toml
[target.'cfg(target_vendor = "apple")'.dependencies]
candle-core = { version = "0.10", optional = true, features = ["metal"] }
```
The `candle-metal` feature now uses `candle-core?/metal` syntax — the `?` means "enable metal on candle-core only if it's already an active dependency". On Linux, the target-specific dep doesn't exist, so this is a no-op. On Apple, `objc2` is pulled in and compiles successfully.

**Key insight**: `--all-features` is a legitimate test configuration. Features that enable platform-specific native code should use target-specific `[target.'cfg(...)'.dependencies]` rather than relying on `compile_error!` guards or mutually-exclusive feature checks.

## Pending

- Feature 03-http-wasm-compat: ServeWriter trait, Server enum, wasm bridge
- Feature 05-db-wasm-compat: CF D1/R2/KV wasm-bindgen bridge
- Feature 06-example-app: Working login app
- Feature 07-ci-wasm-checks: CI wasm compilation checks
- Feature 08-wasm-oauth-manager: wasm-bindgen OAuth manager
- Feature 09-wasm-testbed: CLI-driven wasm test harness
