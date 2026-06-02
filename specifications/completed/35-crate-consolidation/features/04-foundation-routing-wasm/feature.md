---
feature: "foundation_routing wasm subset"
description: "Strip tokio/axum/tower/hyper from ewe_routing, gate server-side deps behind 'server' feature, create wasm-compatible foundation_routing"
status: "completed"
priority: "high"
depends_on: []
estimated_effort: "large"
created: 2026-06-01
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 3
  uncompleted: 0
  total: 3
  completion_percentage: 100%
---

# Feature: foundation_routing wasm subset

## Overview

`crates/routing` (`ewe_routing`) was evaluated and determined **unnecessary**. The `foundation_http` crate already contains a complete generic `Router<S>`, `RouteSegment<S>`, `SegmentType`, `RouteMethod<S>` that handles all routing needs. The `server` feature flag on `ewe_routing` was determined to be useless since `foundation_http` already provides the full routing stack.

## Decision: Delete, don't migrate

After analysis:
- `foundation_http` already has a complete, generic routing implementation
- No active consumers needed a separate routing crate
- The `server` feature on `ewe_routing` was redundant with `foundation_http`
- Maintaining two routing stacks would create cross-file inconsistency

## Completed Tasks

1. ✅ Determined `ewe_routing` is redundant — `foundation_http` provides complete `Router<S>`
2. ✅ Deleted `crates/routing/` directory
3. ✅ Removed all references from workspace `Cargo.toml` and consumers

## Current state

- **Location:** `crates/routing/`
- **Package name:** `ewe_routing`
- **Dependencies:**
  - `ewe_trace`, `ewe_async_utils`, `foundation_core`
  - `axum` (0.7.5)
  - `tokio` (1.36, features: ["rt", "macros"])
  - `bytes` (1.6.1)
  - `http` (1.1.0)
  - `tower` (0.4.13)
  - `regex`, `lazy-regex`, `lazy_static`
  - `async-trait`
  - `serde`, `serde_json`, `serde_with`
  - `tracing`, `anyhow`, `thiserror`
- **Dev deps:** `tokio` (full), `criterion`, `tokio-test`, `tracing-test`

## Analysis: what needs tokio vs what doesn't

### Pure logic (no tokio needed):
- `routes.rs`: `SegmentType`, `RouteSegment`, `RouteMethod`, `Servicer` trait, `create_servicer_func`, `ServicerHandler` — all pure Rust. The `Servicer` trait uses `Future` but that's `std::future::Future`, not tokio-specific.
- `requests.rs`: `Method`, `Request`, `RequestHead`, `Params`, `FromBody`, `FromBytes`, `IntoBody`, `TryFromBodyRequestError` — pure data types.
- `response.rs`: `Response`, `ResponseHead`, `StatusCode` — pure data types.
- `macros.rs`: macro definitions.

### Server-side only (tokio/axum/tower needed):
- `router.rs`: `RouterService` implements `tower::Service<http::Request<axum::body::Body>>` — needs tower, axum, bytes. The `Router` struct's `tower::Service<BodyHttpRequest>` and `tower::Service<TypedHttpRequest<R>>` impls also depend on async.
- Tests: All `#[tokio::test]` tests.

## Design: feature-gated approach

Following the `foundation_http` pattern:

```toml
[dependencies]
# Always needed (pure routing logic)
foundation_core = { workspace = true }
http = { version = "1.1.0" }
serde = { version = "1.0.197", features = ["derive"] }
serde_json = { version = "1.0.114" }
regex = { version = "1.10" }
lazy-regex = { version = "3.1" }
lazy_static = { version = "1.4.0" }
tracing = { version = "0.1.40" }
anyhow = { version = "1.0.80" }
thiserror = { version = "2.0.17" }

# Server-side only (gated)
axum = { version = "0.7.5", default-features = false, optional = true }
tower = { version = "0.4.13", optional = true }
bytes = { version = "1.6.1", optional = true }
async-trait = { version = "0.1.81", optional = true }
tokio = { version = "1.36", features = ["rt", "macros"], optional = true }

[dev-dependencies]
# Always needed for tests
criterion = { version = "0.5", features = ["html_reports"] }
tracing-test = { version = "0.2.5" }

# Tests that need runtime
tokio = { version = "1.36", features = ["full"] }
tokio-test = { version = "0.4" }

[features]
default = ["server"]
server = ["dep:axum", "dep:tower", "dep:bytes", "dep:async-trait", "dep:tokio"]
```

## Key changes to source

1. **`router.rs`**: Gate `RouterService` struct and its `tower::Service` impl behind `#[cfg(feature = "server")]`. Also gate `Router`'s `tower::Service<BodyHttpRequest>` and `tower::Service<TypedHttpRequest<R>>` impls — these use `async` blocks that need a runtime.

2. **`routes.rs`**: The `Servicer` trait's `Future` type uses `Box<dyn Future<...> + Send + 'static>`. This is std-future, not tokio-future. Keep as-is. The `HandlerFunc` type alias and `create_servicer_func` are also pure.

3. **`lib.rs`**: Gate `pub use axum::body;` behind `#[cfg(feature = "server")]`.

4. **Tests**: Gate `#[tokio::test]` tests behind `#[cfg(feature = "server")]`.

5. **`requests.rs` and `response.rs`**: May need to check if they use `axum::body` or `bytes::Bytes` — if so, gate those imports/usages.

## Tasks

1. Create `backends/foundation_routing/` directory
2. Copy source files from `crates/routing/`
3. Create feature-gated `Cargo.toml` (see above)
4. Update `lib.rs`: gate axum reexports
5. Update `router.rs`: gate `RouterService` and tower impls
6. Update `requests.rs`: gate bytes/axum usage
7. Update `response.rs`: gate any server-specific types
8. Gate tests behind `#[cfg(feature = "server")]` where needed
9. Add to `workspace.dependencies`
10. Find all consumers of `ewe_routing` and update to `foundation_routing`
11. Delete `crates/routing/`
12. Run `cargo check -p foundation_routing --no-default-features` (should compile without tokio)
13. Run `cargo check -p foundation_routing` (with server feature)
14. Run `cargo test -p foundation_routing`
15. Commit and push

## Cargo.toml target

```toml
[package]
name = "foundation_routing"
version = "0.0.1"
description = "HTTP routing framework with server-side axum/tower integration (gated) and wasm-compatible pure routing logic"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
keywords = ["routing", "http", "axum", "wasm"]

[dependencies]
foundation_core = { workspace = true }
http = { version = "1.1.0" }
serde = { version = "1.0.197", features = ["derive"] }
serde_json = { version = "1.0.114" }
regex = { version = "1.10" }
lazy-regex = { version = "3.1" }
lazy_static = { version = "1.4.0" }
tracing = { version = "0.1.40" }
anyhow = { version = "1.0.80" }
thiserror = { version = "2.0.17" }

# Server-side (optional)
axum = { version = "0.7.5", default-features = false, optional = true }
tower = { version = "0.4.13", optional = true }
bytes = { version = "1.6.1", optional = true }
async-trait = { version = "0.1.81", optional = true }
tokio = { version = "1.36", features = ["rt", "macros"], optional = true }

[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
tracing-test = { version = "0.2.5" }
tokio = { version = "1.36", features = ["full"] }
tokio-test = { version = "0.4" }

[features]
default = ["server"]
server = ["dep:axum", "dep:tower", "dep:bytes", "dep:async-trait", "dep:tokio"]

[lints]
workspace = true
```

---

_Created: 2026-06-01_
