# Learnings: Cloudflare Workers Readiness

## Completed Work

### 01-core-wasm-compat (2026-05-15)
- Removed dead `ctrlc` dependency from `foundation_core`
- Added `wasm` feature flag for `uuid/js`, `chrono/wasmbind`, `getrandom/js`
- SSL backend switching: `aws-lc-rs` (wasm) vs `ring` (native)

### 03-http-wasm-compat (2026-05-31)
- Full module restructure: `shared/` (traits, router, middleware, handlers, app), `native/` (TCP server, reader, upgrade), `wasm/` (dispatch, streams, bridge)
- `Serve` (native-only), `ServeWriter` (both targets), `WebServe` (async, web), `CfServe` (async, CF Workers) traits
- `CfHttpApp` and `WasmHttpApp` wasm-bindgen wrappers with `handleRequest` entry points
- `CfConn` and `WebConn` typed connection types with `into_response()` → `web_sys::Response`
- Router extended with `add_route_cf`, `add_route_web`, `add_route_writer` methods
- `HttpApp` builder with `route_cf`, `route_web`, `route_writer`, `new_cf`, `new_web`
- `dispatch.rs` with `HttpAppCfDispatch` and `HttpAppWebDispatch` extension traits
- **Cfg fix**: `CfConn::into_response()` and `WebConn::into_response()` must use `#[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-http"))]`, not just `#[cfg(target_arch = "wasm32")]`, because `web_sys` and `wasm_bindgen` are optional deps only pulled in by the `wasm-bindgen-http` feature

### 04-wire-restructure (2026-05-16)
- Split `foundation_core::wire` modules into shared (always compiled) and native (`#[cfg(not(wasm32))]`) submodules
- Deep client split in `simple_http`: moved pure logic to `client/shared/`, socket-dependent code to `client/native/`
- `event_source` and `websocket` shared types extracted, native consumers gated
- `http_stream` entirely native (uses `RawStream`)

### 05-db-wasm-compat (2026-05-31)
- Module restructure complete: `core/` (traits, errors, schema, memory backends, HTTP-based D1/R2), `native/` (turso, libsql), `wasm/` (bindgen/cf D1/R2/KV, wasm_storage)
- D1/R2/KV wasm-bindgen types exist in `wasm/bindgen/cf/` with `from_env()` extraction
- **Key fix**: `foundation_netio` `native/` submodules were gated with `#[cfg(feature = "multi")]` but NOT `not(target_arch = "wasm32")`. Since `d1` pulls in `multi`, foundation_db couldn't compile on wasm32. Fixed in 4 files: `simple_http/client/mod.rs`, `event_source/mod.rs`, `websocket/mod.rs`, `http_stream/mod.rs` (lib.rs) — all now use `#[cfg(all(feature = "multi", not(target_arch = "wasm32")))]`

### 10-generic-serve-traits (2026-05-31)
- `Server` enum replaced with generic `Router<S>`, `RouteMethod<S>`, `RouteSegment<S>` — handler type flows from `HttpApp<S>` through entire route tree
- Environment-specific traits: `Serve` (native TCP), `ServeWriter` (bytes to writer), `CfServe` (CF Workers structured fields), `WebServe` (browser structured fields)
- `CfConn`, `WebConn` typed connection types with `into_response()` → `web_sys::Response` — no wire-format parsing needed
- `HttpApp` builder methods: `route_cf`/`route_web`/`route_writer`, `new_cf`/`new_web`
- Dispatch extension traits: `HttpAppCfDispatch`, `HttpAppWebDispatch` in `wasm/dispatch.rs`

### 13-cf-valtron-counter (2026-05-31)
- `examples/cf-valtron-counter/` — CF Worker with static HTML (`/`) and streaming counter (`/counter`)
- `CounterTaskIterator` alternates `TaskStatus::Wait` / `TaskStatus::Pending` to exercise both executor JS yield branches
- Counter endpoint uses `execute()` + `into_future_stream()` + `.await` → `ReadableStream` via `future_to_promise`
- Verified: `cargo check --target wasm32-unknown-unknown` passes clean

### 06-example-app (2026-05-31)
- `examples/cf-login-app/` fully implemented: register, login, dashboard, logout, note save/retrieve
- Uses direct `async fn fetch(req, env)` entry point with `LazyApp` (OnceLock) for shared state
- All async operations call D1 JS API directly via `JsFuture` — no valtron streams on wasm
- `SessionManager::create_session_async` / `get_session_async` / `revoke_session_async` exercised
- `D1WasmStorage::set_async` / `get_async` / `query_async` / `execute_async` for direct SQL and KV
- minijinja templates for HTML rendering, argon2 password hashing
- Verified: `cargo check --target wasm32-unknown-unknown` passes clean

### 08-wasm-oauth-manager (2026-05-31)
- `WasmOAuth` in `wasm_bindgen/oauth.rs` — full OAuth 2.0 flows via `web_sys::fetch`
- Three token exchange methods: `exchange_code_async`, `client_credentials_async`, `refresh_token_async`
- Sync wrappers (`exchange_code`, `client_credentials`, `refresh_token`) via valtron `exec_future`
- `js_fetch()` runtime-detects ServiceWorkerGlobalScope vs browser Window context
- `wasm-bindgen-oauth` feature flag gates wasm-bindgen deps — lightweight base wasm build unaffected
- Fix: `exec_future` was imported from `foundation_db` but was private — replaced with local valtron-based impl

## Lessons Learned

- **Duplicate numbering causes confusion**: Initially had two `03-` features (`03-http-wasm-compat` and `03-wire-restructure`). Fixed by renumbering sequentially.
- **Orphaned root-level modules**: When restructuring `foundation_http` into `shared/`/`native/`/`wasm/`, old root-level directories were left behind. Deleted them — they were already unreferenced by `lib.rs`.

- **`#[cfg(feature = "multi")]` is NOT enough for wasm32 gating**: When a feature is enabled via `#[cfg(feature = "multi")]` but the code uses `RawStream`, `Connection`, `ssl`, or other native-only types, it will fail on wasm32. Always combine with `not(target_arch = "wasm32")`: `#[cfg(all(feature = "multi", not(target_arch = "wasm32")))]`. This caught 4 modules: `simple_http/client/native`, `event_source/native`, `websocket/native`, `http_stream`.

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

All 13 features complete. See spec 32 (`cf-serve-app`) for the idiomatic `CfHttpAppSingleton` entry point pattern.
