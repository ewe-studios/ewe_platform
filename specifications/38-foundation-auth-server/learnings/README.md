# cf* Projects — Cross-Cutting Learnings Summary

**Projects Reviewed**: 4
**Date**: 2026-06-07

## Project Inventory

| Project | Type | Lines of Code | Key Insight |
|---------|------|---------------|-------------|
| `cf-colo-hint` | Static Rust library | ~3K (generated) | Colo-to-region mapping, zero-dep, const fn |
| `cf-connectrpc-middleware` | Cargo workspace (4 crates) | ~2K Rust | Six middleware surfaces, tower::Layer patterns |
| `cf-do-locator` | CF Worker service + client | ~500 Rust/TS | Isolate-level caching, type-safe DO placement |
| `cf-rauthy` | Strategy document | N/A | Dual-native + CF constraint, OIDC relay vision |

## Top Borrowable Patterns (Ranked by Value)

### 1. Shadow Mode for Safe Rollouts ⭐⭐⭐
**From**: cf-connectrpc-middleware (`Rollout` trait)
**Pattern**: Run new logic alongside existing, log decisions, never act. After zero mismatches, flip to enforce.
**For ewe_platform**: JWT verification rollout — run `JwtVerifier::verify()` alongside existing `JwtToken::from_token()`, compare results, flip when confident.
**Effort**: Low — just the `Rollout` trait + `log_shadow()` helper

### 2. Extractor Trait Pattern ⭐⭐⭐
**From**: cf-connectrpc-middleware (`CedarRequestExtractor`, `CfFieldsExtractor`)
**Pattern**: Middleware doesn't know how to get its input — consumer provides extractor. Decouples from specific types.
**For ewe_platform**: Auth services could use extractor traits instead of hardcoding credential store types. Makes services composable across native and wasm.
**Effort**: Low — just trait definitions

### 3. Hysteresis for Configuration Stability ⭐⭐⭐
**From**: cf-colo-hint, cf-do-locator
**Pattern**: Deadband (≥15ms OR ≥20% improvement) prevents config flapping on noisy measurements.
**For ewe_platform**: JWKS key rotation detection — don't flip to a new key unless latency/performance improvement is significant. Rate limit threshold adjustments.
**Effort**: Low — just the comparison logic

### 4. `no_std` + Zero-Dep Libraries ⭐⭐
**From**: cf-colo-hint
**Pattern**: Pure-data crates avoid all dependencies for maximum wasm/embedded compatibility.
**For ewe_platform**: Our `AsyncQueryStream`, `AsyncListStream`, auth error types — minimize deps for easier wasm compilation.
**Effort**: Medium — requires auditing existing deps

### 5. Kit/Crate Separation ⭐⭐
**From**: cf-connectrpc-middleware (`connectrpc-tower-kit`)
**Pattern**: Extract shared conventions so middleware family has no circular deps. Every middleware depends on the kit, not on each other.
**For ewe_platform**: Extract shared valtron bridge patterns into a kit module. Auth services depend on the kit, not on each other.
**Effort**: Medium — requires refactoring

### 6. Type-Level Enforcement ⭐⭐
**From**: cf-do-locator (`createTenantDO` won't accept request colo)
**Pattern**: Prevent misuse at compile time through type signatures.
**For ewe_platform**: Can't use a user token for admin operations, can't pass sync stream to async context, etc.
**Effort**: Medium — requires careful type design

### 7. Transparent vs Short-Circuit Layer Distinction ⭐⭐
**From**: cf-connectrpc-middleware (MIDDLEWARES.md)
**Pattern**: Clear architectural distinction between layers that never reject vs layers that may. Different tower::Service bounds, different Future types.
**For ewe_platform**: Our auth middleware stack should classify each layer as transparent (tracing, request enrichment) or short-circuit (authz, rate limit).
**Effort**: Low — just documentation and type discipline

### 8. Isolate-Level Caching with OnceLock ⭐
**From**: cf-do-locator, cf-colo-hint
**Pattern**: Load external data once per isolate, cache forever.
**For ewe_platform**: JWKS, discovery documents, policy files — all boot-once patterns.
**Effort**: Low — just OnceLock usage

### 9. CF Binding Classification ⭐
**From**: cf-connectrpc-middleware (generic / cf-context / cf-binding)
**Pattern**: Three categories for wasm compatibility clarity.
**For ewe_platform**: Classify our wasm modules the same way — helps consumers understand what CF resources they need.
**Effort**: Low — just documentation

### 10. Dual-Native + CF Constraint ⭐
**From**: cf-rauthy
**Pattern**: All ideas must support both native Rust and CF Workers — developers can always fully self-host.
**For ewe_platform**: This should be a design principle for all our auth services.
**Effort**: N/A — just a principle to follow

## Cross-Project Observations

1. **Hysteresis appears in both colo-mapping projects** — this is a general pattern for any noisy measurement system. Worth documenting in our coding guidelines.

2. **All projects share the same author** (joeblew999 / Connor Hindley contributions) — consistent patterns across the family.

3. **Cloudflare Workers as the primary target** — every project is designed for `wasm32-unknown-unknown`. The patterns here are directly applicable to our CF Workers deployment.

4. **ConnectRPC as the service-to-service protocol** — zero-hop worker-to-worker communication, cross-language clients. Consider for our IdP server endpoints.

5. **Cedar as the policy engine** — confirmed wasm-native, microsecond decisions, runs in Workers. Our Feature 14 is on the right track.

6. **Codegen from live data** — both colo-mapping projects use external APIs to generate static Rust code. Could be adapted for our policy config generation.

## Not Applicable / Not Worth Borrowing

- **Rauthy-specific integration** (cf-rauthy #1-6) — requires Rauthy maintainer discussions, outside our scope
- **Hiqlite / SQLite replication** — interesting but our foundation_db already has Turso/libsql/D1
- **CF Email Service integration** — outside our auth scope
- **axum::middleware::from_fn** — explicitly NOT usable on wasm32

## Recommendation for ewe_platform

**Implement first (low effort, high value):**
1. `Rollout` trait + shadow mode for JWT verification rollout
2. Extractor traits for auth service composability
3. Hysteresis for JWKS key rotation detection

**Implement second (medium effort):**
4. Kit module for shared valtron patterns
5. Type-level enforcement for auth token misuse prevention
6. Transparent vs short-circuit documentation for middleware stack

**Keep as reference:**
7-10: Documentation-level patterns, follow when designing new features.
