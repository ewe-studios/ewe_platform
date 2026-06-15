# cf-connectrpc-middleware — Learnings Review

**Source**: `/home/darkvoid/Boxxed/@formulas/src.rust/src.gedweb/cf-connectrpc-middleware/`
**Reviewed**: 2026-06-07

## Project Overview

ConnectRPC middleware for Cloudflare Workers in Rust. Built to compose with `connyay/connectrpc-workers` (server-side ConnectRPC runtime for Workers). Focus areas: Cedar policy authorization, CF tracing, rate limiting, metrics.

**Architecture**: Cargo workspace with 4 shipped crates + planned ones:

| Crate | Surface | CF Status | Status |
| --- | --- | --- | --- |
| `connectrpc-tower-kit` | Shared primitives | generic | shipped |
| `connectrpc-cedar` | tower::Layer (short-circuit) | generic | shipped |
| `connectrpc-cf-tracing` | tower::Layer (transparent) | cf-context | shipped |
| `connectrpc-cf-rate-limit` | tower::Layer (short-circuit) | cf-binding: Rate Limiting | shipped |
| `connectrpc-cf-metrics` | tower::Layer (transparent) | cf-binding: Analytics Engine | shipped |

## Key Implementation Details

### Six Middleware Surfaces in ConnectRPC

The project identified **six** surfaces for ConnectRPC middleware — critical architectural insight:

1. **`tower::Layer<S>` transparent** — wraps service, pass-through. For enriching requests (extensions, headers)
2. **`tower::Layer<S>` short-circuit** — pins `Response = Response<ConnectRpcBody>`, `Error = Infallible`. For rejecting before envelope decode (authz, rate limit, CORS)
3. **`connectrpc::Interceptor`** — body-aware, registered via `ConnectRpcService::with_interceptor`. Not yet published in connectrpc 0.4
4. **`ConnectRpcService` config** — limits, deadline policies. Already built-in, don't write
5. **Handler-side helper** — called inside handler body for fine-grained authz
6. **Proc-macro handler decorator** — `#[connect_impl]` attribute for per-handler compile-time checks

**Transparent vs short-circuit** matters: Connect encodes failures into response body, not error channel. So `Error = Infallible` for short-circuit layers.

### `connectrpc-tower-kit` — Shared Primitives

No middleware lives here — only conventions:

- **`Rollout` trait**: Generalizes shadow/enforce pattern so every rejecting middleware adopts it with its own enum (`Shadow/Enforce`, `Observe/Throttle`, `Warn/Reject`)
- **`log_shadow()`**: Centralized shadow log line format (`target = "connectrpc_middleware"`) so operators can grep once
- **`ShortCircuitFuture`**: `pin_project_lite` Future enum (`Pass { inner }` / `Denied { response }`) for any short-circuit layer
- **`deny_response()`**: Build Connect-protocol error response consistently
- **`ext`**: Canonical names for `req.extensions()` entries so middlewares compose

### `connectrpc-cedar` — Cedar Policy Authorization

```
request → AuthLayer (verifies) → CedarLayer (authorizes) → service (business logic)
```

- **`CedarLayer::shadow()`** — evaluates + logs, never rejects. For parallel rollout with existing hand-rolled authz
- **`CedarLayer::enforce()`** — evaluates + rejects on `Decision::Deny`
- **`skip_paths()`** — builder for public endpoints (healthz, OAuth callbacks)
- Reads `SessionContext` from request extensions, maps URL path to Cedar Action
- Zero DB lookup at authorization time — macaroon session pins scope, Cedar evaluates action
- `CedarRequestExtractor` trait for consumer to define session → Cedar request mapping

### `connectrpc-cf-tracing` — CF-Aware Tracing

- Transparent layer (never rejects)
- Opens `tracing::Span` around every Connect-RPC call
- Carries CF metadata: colo, country, ASN, TLS cipher, HTTP protocol, cf-ray
- Consumer provides `CfFieldsExtractor` closure — no direct CF dep in crate

### `connectrpc-cf-rate-limit` — Rate Limiting

- Short-circuit layer calling Cloudflare Rate Limiting binding
- `Mode::Observe` (call binding, log, never block) / `Mode::Enforce` (block on exceeded)
- Consumer implements `RateLimiter` trait wrapping their binding — same pattern as tracing extractor
- `IpKeyExtractor` + `RateLimitKeyExtractor` traits for flexible keying

## Borrowable Patterns

### 1. Kit/Crate Separation (connectrpc-tower-kit)
Extract shared conventions into a separate crate so middleware family doesn't have circular deps. Every middleware depends on the kit, not on each other. For ewe_platform: we could extract shared valtron bridge patterns into a `foundation_core::valtron::kit` module.

### 2. Rollout Trait for Safe Rollouts
Instead of hardcoding `Mode::Shadow/Enforce` in every middleware, define a `Rollout` trait:
```rust
pub trait Rollout: Debug + Send + Sync + 'static {
    fn is_enforcing(&self) -> bool;
    fn name(&self) -> &'static str;
}
```
Each middleware defines its own enum and impls `Rollout`. For ewe_platform: we could use this for feature flags on new auth features — run new verification logic in shadow mode before enforcing.

### 3. CF Binding Classification
Three categories for CF Workers compatibility:
- **`generic`** — no CF-specific data, no CF binding. Works on any tower host
- **`cf-context`** — reads CF Workers runtime data but declares no binding
- **`cf-binding: <kind>`** — declares a CF binding consumer must provision

For ewe_platform: we should classify our wasm modules the same way — generic wasm vs CF-binding-dependent.

### 4. Transparent vs Short-Circuit Layer Distinction
Clear architectural distinction between layers that never reject (tracing, request enrichment) vs layers that may reject (authz, rate limit). Different `tower::Service` bounds, different Future types.

### 5. Shadow Mode for Safe Rollouts
Run new middleware in parallel with existing logic, log decisions but don't act. Operators compare shadow logs against actual responses. After zero mismatches, flip to enforce. For ewe_platform: we could use this when adding new JWT verification — run alongside existing unverified parsing first.

### 6. Extractor Trait Pattern
Middleware doesn't know how to get its input — consumer provides an extractor closure/trait:
```rust
pub trait CedarRequestExtractor<B>: Send + Sync + 'static {
    fn extract(&self, req: &http::Request<B>) -> Option<CedarRequest>;
}
```
Decouples middleware from specific session types. For ewe_platform: our auth services could use extractor traits instead of hardcoding credential store types.

### 7. Canonical Extension Names
Document convention for `req.extensions()` keys so middlewares compose without key collisions:
```rust
pub const SESSION_CONTEXT: &str = "connectrpc:session";
pub const REQUEST_ID: &str = "connectrpc:request-id";
```
For ewe_platform: we could use this for valtron stream metadata keys.

### 8. `Error = Infallible` for Short-Circuit Layers
Connect encodes failures into response body, not error channel. Short-circuit layers follow suit — `type Error = Infallible`, construct `Response<ConnectRpcBody>` on denial. Important pattern for any protocol that encodes errors in body.

## Applicability to ewe_platform

**Directly useful:**
- The `Rollout` trait pattern for safe feature rollouts — run new auth logic in shadow mode first
- The extractor trait pattern — our auth services could use extractors instead of hardcoding types
- The transparent vs short-circuit distinction — applies to our middleware stack design
- The CF binding classification — useful for categorizing our wasm modules

**Conceptually useful:**
- The kit/crate separation philosophy — extract shared patterns so crates don't depend on each other
- The shadow mode approach — test new JWT verification alongside existing code before enforcing
- The six surfaces analysis — helps us understand where our auth middleware should sit in the tower stack

## Potential Issues Noted

1. The `connectrpc::Interceptor` surface (body-aware authz) is not yet published in connectrpc 0.4
2. `axum::middleware::from_fn` doesn't compile on `wasm32-unknown-unknown` — must hand-roll tower::Layer
3. Short-circuit layers require `Error = Infallible` pin, which constrains the inner service type
4. The kit uses `pin_project_lite` which can't doc-comment generated struct fields (known limitation)

---
*This review covers the workspace architecture and key crates. For full details, see MIDDLEWARES.md in the repo.*
