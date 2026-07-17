---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F07-cache-tiers"
this_file: "specifications/52-tauri-foundation-platform/features/F07-cache-tiers/feature.md"

status: pending
priority: high
created: 2026-07-17

depends_on:
  - "F01-session-backbone"

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# F07 — Cache tiers and per-route cache policies

## Overview

Implement the two-tier offline model: Tier 1 local WASM execution (offline by
default), Tier 2 rendered page caching (remote content cached in SQLite).
Cache policies are per-route, assigned in `RouteDecision.cache_policy`.

[Decision 05](../decisions/05-offline-and-sync.md) defines the cache model.

## Dependencies

Depends on:
- `F01-session-backbone` — Cache manager lives on the session

Required by:
- `F08-walking-skeleton` — Cache check in navigation flow

## Requirements

### 1. `CacheManager`

```rust
// foundation_platform/src/cache.rs

pub struct CacheManager {
    db: foundation_db::Database,  // SQLite
}

impl CacheManager {
    pub fn new(db_path: &Path) -> Result<Self> { ... }

    /// Store a cached response, keyed by route
    pub fn store(&self, route: &str, response: &CachedResponse) -> Result<()> { ... }

    /// Retrieve a cached response for a route, if available
    pub fn get(&self, route: &str) -> Option<CachedResponse> { ... }

    /// Invalidate cached content for specific routes
    pub fn invalidate(&self, routes: &[&str]) -> Result<()> { ... }

    /// Check if cache should serve this request (based on policy)
    pub fn should_serve_cached(&self, decision: &RouteDecision, route: &str) -> bool { ... }
}

struct CachedResponse {
    body: Vec<u8>,
    content_type: String,
    cached_at: DateTime<Utc>,
    profile: Profile,  // Cache is profile-scoped
}
```

### 2. `CachePolicy` — per-route control

From [decision 05](../decisions/05-offline-and-sync.md):

| Policy | Behavior | Offline behavior |
|---|---|---|
| `CacheFirst` | Serve from cache. Fetch on miss. | Works offline |
| `NetworkFirst` | Try network. Fall back to cache. | Serves stale cache |
| `OnlineOnly` | Never cache. Always network. | Fails with offline error |
| `LocalOnly` | Always cache. Never network. | Always works |
| `StaleWhileRevalidate` | Serve cache, refresh in background. | Serves stale cache |

```rust
impl CacheManager {
    pub fn should_serve_cached(&self, decision: &RouteDecision, route: &str) -> bool {
        let has_cache = self.get(route).is_some();
        match decision.cache_policy {
            CachePolicy::CacheFirst | CachePolicy::LocalOnly => has_cache,
            CachePolicy::StaleWhileRevalidate => has_cache, // serve cache, revalidate in background
            CachePolicy::NetworkFirst | CachePolicy::OnlineOnly => false,
        }
    }
}
```

### 3. Cache storage

Protocol-transparent storage: if the server returned
`application/primal-columnar`, the cache stores the columnar bytes. On replay,
it returns the exact same bytes with the same Content-Type.

### 4. Connectivity lifecycle

The session tracks connectivity state and reacts for caching:

```
Online → Cache serves fresh content
Background → Cache preserved, no eviction
Offline → Cache serves stale (CacheFirst/NetworkFirst/StaleWhileRevalidate)
          OnlineOnly routes show offline error
Reconnected → Cache invalidates stale entries (server decides which)
```

### 5. Platform vs backend boundary

Platform provides: SQLite storage, `CachePolicy` enum, `invalidate()` triggers.
Backend decides: what to cache, when to invalidate, update strategy (full
replace or surgical update).

## Tasks

### CacheManager
- [ ] Create `src/cache.rs` with `CacheManager` struct
- [ ] Open SQLite database at session initialization
- [ ] Implement `store()` — insert or replace cached response by route
- [ ] Implement `get()` — retrieve cached response by route
- [ ] Implement `invalidate()` — remove cached entries by route list

### CachePolicy enforcement
- [ ] Implement `should_serve_cached()` based on policy
- [ ] Test: CacheFirst serves cache, fetches on miss
- [ ] Test: NetworkFirst skips cache, falls back on network failure
- [ ] Test: OnlineOnly never caches
- [ ] Test: LocalOnly never fetches
- [ ] Test: StaleWhileRevalidate serves cache immediately, revalidates async

### Profile scoping
- [ ] Cache entries scoped to Profile (App/TrustedRemote/UntrustedRemote isolated)
- [ ] Test: UntrustedRemote cannot read App profile's cached content

### Connectivity integration
- [ ] Wire cache behavior to online/offline session events
- [ ] Test: offline → OnlineOnly routes error, CacheFirst routes serve cached

### Protocol transparency
- [ ] Store responses as-encoded (same Content-Type, same bytes)
- [ ] Test: columnar response cached and replayed identically

## Verification Commands

```bash
cargo test --package foundation_platform -- cache
```
