---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F07-cache-tiers"
this_file: "specifications/52-tauri-foundation-platform/features/F07-cache-tiers/feature.md"

status: completed
priority: high
created: 2026-07-17
updated: 2026-07-21

depends_on:
  - "F01-session-backbone"

tasks:
  completed: 4
  uncompleted: 0
  total: 4
  completion_percentage: 100%
---

# F07 — Cache tiers and per-route policies

## Overview

Implement `CacheManager` (SQLite-backed, protocol-transparent), per-route
`CachePolicy` enforcement, profile-scoped cache isolation, and connectivity
lifecycle integration. Two-tier offline: Tier 1 local WASM (offline by
default), Tier 2 rendered page caching (remote content cached locally).

[Decision 05](../decisions/05-offline-and-sync.md).

---

## Part A — CacheManager

```rust
// foundation_platform/src/cache.rs

pub struct CacheManager {
    db: foundation_db::Database,
}

pub struct CachedResponse {
    pub body: Vec<u8>,
    pub content_type: String,
    pub cached_at: i64,         // Unix timestamp
    pub profile: Profile,       // Profile-scoped
}

impl CacheManager {
    pub fn new(db_path: &str) -> Self { ... }
    pub fn store(&self, route: &str, response: &CachedResponse) -> Result<()> { ... }
    pub fn get(&self, route: &str) -> Option<CachedResponse> { ... }
    pub fn invalidate(&self, routes: &[&str]) -> Result<()> { ... }
    pub fn flush(&self) { ... }

    /// Determine whether the cache should serve this request.
    pub fn should_serve_cached(&self, decision: &RouteDecision, route: &str) -> bool {
        let has_cache = self.get(route).is_some();
        match decision.cache_policy {
            CachePolicy::CacheFirst | CachePolicy::LocalOnly => has_cache,
            CachePolicy::StaleWhileRevalidate => has_cache, // serve, then async revalidate
            CachePolicy::NetworkFirst | CachePolicy::OnlineOnly => false,
        }
    }
}
```

### A.2 — CachePolicy enforcement table

| Policy | Online | Offline |
|---|---|---|
| CacheFirst | Serve cache. Fetch on miss. | Works (cached content) |
| NetworkFirst | Try network. Fall back to cache. | Serves stale cache |
| OnlineOnly | Never cache. Always network. | Fails with offline error |
| LocalOnly | Always cache. Never network. | Always works |
| StaleWhileRevalidate | Serve cache, refresh async. | Serves stale cache |

### A.3 — Protocol transparency

Responses stored as-encoded: same Content-Type, same bytes. Replay is
identical to origin. The cache is protocol-transparent.

### A.4 — Profile scoping

Cache entries are scoped to `Profile`. `UntrustedRemote` pages cannot
read cached content from `App` or `TrustedRemote` routes.

---

## Verification

```bash
cargo test --package foundation_platform -- cache
```
