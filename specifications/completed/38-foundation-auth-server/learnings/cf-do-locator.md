# cf-do-locator — Learnings Review

**Source**: `/home/darkvoid/Boxxed/@formulas/src.rust/src.gedweb/cf-do-locator/`
**Reviewed**: 2026-06-07

## Project Overview

A Cloudflare Worker service + client library that places every Durable Object near its user automatically. Solves the "pick the right region at DO creation time" problem — DOs live in one region forever, CF doesn't migrate them.

**Two deployment modes**: 
1. **Service** — deployed Worker that serves colo-to-region mappings over RPC
2. **Client library** — vendored TypeScript/Rust helper (~200 lines, zero deps)

**Live demo**: https://cf-do-locator.gedw99.workers.dev

## Architecture

```
Upstream sources (weekly refresh):
  cloudflarestatus.com/api/v2/components.json  → colo list
  where.durableobjects.live/api/v3/data.json   → measured latency per colo

  ↓ refresh.rs (runs on Worker via cron)

Cloudflare KV namespace:
  colo → region mapping table
  colo → full info (city, country, latency data)

  ↓ RPC service (JSON over POST)

Consumer Workers:
  Boot cache once per isolate (OnceLock / LocatorCache)
  Route every DO creation through funnel helpers
```

## Key Implementation Details

### RPC Service Contract

Endpoints (all JSON POST):
- `GET /healthz` — health check
- `POST /__refresh` — force refresh from upstream (auto-runs Mondays 03:00 UTC)
- `POST /locator.v1.LocatorService/GetLocationHint` → `{"hint":"wnam","known":true}`
- `POST /locator.v1.LocatorService/GetColoInfo` → full info for one colo
- `POST /locator.v1.LocatorService/ListColos` → every colo (consumers cache at boot)
- `POST /locator.v1.LocatorService/GetSnapshot` → metadata (version, counts)

### Weekly Refresh with Hysteresis

1. Download colo list from cloudflarestatus.com
2. Download measured latency per colo per DO region from where.durableobjects.live
3. For each colo, pick region with lowest latency
4. **Hysteresis**: if winning region changed, only accept if ≥15ms (or ≥20%) faster. Stops table flapping between near-tied regions
5. Write new table to KV

Same hysteresis algorithm as `cf-colo-hint` — shared code from Connor Hindley's work.

### Funnel Helpers (Type Safety)

The client library enforces correct colo usage at the type level:

- **`createUserDO(cache, request.cf.colo, ...)`** — per-user data, uses arrival edge colo
- **`createTenantDO(ownerRegion, ...)`** — per-org data, takes explicit region. **Does NOT accept request colo** — prevents admin provisioning wrong region
- **`createSessionDO(cache, request.cf.colo, ...)`** — short-lived, creator's edge ≈ usage edge

Critical footgun documented: `idFromName` does NOT honor `locationHint` if any DO with that name exists. Only `newUniqueId({ locationHint })` actually places.

### Rust Client Pattern

```rust
static CACHE: OnceLock<LocatorCache> = OnceLock::new();

fn locator(env: &worker::Env) -> worker::Result<&'static LocatorCache> {
    Ok(CACHE.get_or_init(|| {
        let url = env.var("CF_DO_LOCATOR_URL").unwrap().to_string();
        let mut c = LocatorCache::new(url);
        c.load().expect("locator boot");
        c
    }))
}
```

Single `OnceLock` per isolate, boot at first request, cache forever.

## Borrowable Patterns

### 1. Isolate-Level Caching with OnceLock
Load external data once per isolate, cache forever. For ewe_platform: JWKS, discovery documents, policy files — all boot-once patterns.

### 2. Type-Level Enforcement of Correct Usage
`createTenantDO` won't accept a request colo — compile-time enforcement of "don't use admin's edge for org provisioning". For ewe_platform: we could use type-level markers to prevent misuse of auth credentials (e.g., can't use a user token for admin operations).

### 3. Hysteresis for Configuration Stability
The 15ms/20% deadband prevents configuration tables from flapping. For ewe_platform: our JWKS key rotation detection, rate limit threshold adjustments, or any config that regenerates from noisy measurements.

### 4. Service + Vendored Client Pattern
Deploy a shared service, but also provide a zero-dep vendored client (~200 lines). Users who don't want the service dependency can vendor the static library alternative. Same pattern as `cf-colo-hint` (static enum library).

### 5. `idFromName` vs `newUniqueId` Footgun Documentation
Documented the critical difference: `idFromName` ignores `locationHint` if DO already exists, `newUniqueId` honors it. For ewe_platform: we should document similar footguns in our async patterns (e.g., `collect_one` on multi-row streams).

## Applicability to ewe_platform

**Directly useful:**
- The OnceLock per-isolate caching pattern for JWKS, discovery docs, policy files
- The type-level enforcement pattern — prevent misuse at compile time
- The hysteresis algorithm for stable config regeneration

**Conceptually useful:**
- The service + vendored client tradeoff — for our auth library, consumers could use a service or a static library
- The footgun documentation approach — explicitly document what not to do

## Potential Issues Noted

1. The service depends on external APIs (cloudflarestatus.com, where.durableobjects.live) — if they change, refresh breaks
2. KV namespace IDs are not secrets but must be provisioned before deploy
3. The Rust client uses `ureq` for HTTP — doesn't compile to wasm32, must swap for `worker::Fetch`
4. No retry logic for refresh failures — if the weekly refresh fails, stale data persists

---
*Related: cf-colo-hint provides the same mapping as a static Rust library (no service dep). Same upstream data, same hysteresis algorithm.*
