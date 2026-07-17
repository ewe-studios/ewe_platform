//! Cache tiers — per-route cache with foundation_db StorageProvider backend.
//!
//! Two-tier offline model from decision 05:
//!   Tier 1: Local WASM execution (offline by default — no cache needed)
//!   Tier 2: Rendered page caching (remote content cached locally)
//!
//! Protocol-transparent: stores responses as-encoded (same bytes, same
//! Content-Type). Replay is identical to the original response.
//!
//! Profile-scoped: UntrustedRemote pages cannot read App cache entries.
//! Backends: MemoryCacheStorage (tests), DbCacheStorage (foundation_db).

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use foundation_ui_traits::*;

use crate::session::PlatformSession;

// ── Storage trait ────────────────────────────────────────────────────

/// Backend storage for cached responses. Default is MemoryCacheStorage.
/// Swappable for foundation_db::StorageProvider via DbCacheStorage.
pub trait CacheStorage: Send + Sync + 'static {
    fn get(&self, key: &str) -> Option<CachedEntry>;
    fn set(&self, key: &str, entry: CachedEntry);
    fn remove(&self, key: &str);
    fn keys(&self) -> Vec<String>;
}

/// In-memory storage. Used for tests and as the default backend.
#[derive(Default)]
pub struct MemoryCacheStorage {
    data: RwLock<HashMap<String, CachedEntry>>,
}

impl MemoryCacheStorage {
    pub fn new() -> Self {
        Self { data: RwLock::new(HashMap::new()) }
    }
}

impl CacheStorage for MemoryCacheStorage {
    fn get(&self, key: &str) -> Option<CachedEntry> {
        self.data.read().unwrap().get(key).cloned()
    }
    fn set(&self, key: &str, entry: CachedEntry) {
        self.data.write().unwrap().insert(key.to_string(), entry);
    }
    fn remove(&self, key: &str) {
        self.data.write().unwrap().remove(key);
    }
    fn keys(&self) -> Vec<String> {
        self.data.read().unwrap().keys().cloned().collect()
    }
}

// ── Cached entry ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CachedEntry {
    pub body: Vec<u8>,
    pub content_type: String,
    pub cached_at: u64,       // Unix timestamp (seconds)
    pub profile: Profile,
}

impl CachedEntry {
    pub fn new(body: Vec<u8>, content_type: &str, profile: Profile) -> Self {
        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self { body, content_type: content_type.to_string(), cached_at, profile }
    }
}

// ── Cache manager ────────────────────────────────────────────────────

/// Protocol-transparent cache storage with per-route policy enforcement.
/// Profile-scoped — entries are keyed by `{profile}:{route}`.
pub struct CacheManager {
    storage: Box<dyn CacheStorage>,
}

impl CacheManager {
    pub fn new(storage: impl CacheStorage) -> Self {
        Self { storage: Box::new(storage) }
    }

    pub fn in_memory() -> Self {
        Self::new(MemoryCacheStorage::new())
    }

    fn scoped_key(profile: Profile, route: &str) -> String {
        format!("{:?}:{route}", profile)
    }

    pub fn store(&self, profile: Profile, route: &str, body: &[u8], content_type: &str) {
        let key = Self::scoped_key(profile, route);
        self.storage.set(&key, CachedEntry::new(body.to_vec(), content_type, profile));
    }

    pub fn get(&self, profile: Profile, route: &str) -> Option<CachedEntry> {
        self.storage.get(&Self::scoped_key(profile, route))
    }

    pub fn invalidate_routes(&self, routes: &[&str]) {
        let all_keys = self.storage.keys();
        for key in all_keys {
            for route in routes {
                if key.ends_with(route) {
                    self.storage.remove(&key);
                }
            }
        }
    }

    pub fn invalidate(&self, profile: Profile, route: &str) {
        self.storage.remove(&Self::scoped_key(profile, route));
    }

    /// Check whether the cache should serve this request.
    ///
    /// Returns `true` when: CacheFirst + has entry, LocalOnly + has entry,
    /// StaleWhileRevalidate + has entry. Returns `false` for NetworkFirst
    /// and OnlineOnly (go to network regardless).
    pub fn should_serve_cached(&self, decision: &RouteDecision, route: &str) -> bool {
        let has_entry = self.get(decision.profile, route).is_some();
        match decision.cache_policy {
            CachePolicy::CacheFirst | CachePolicy::LocalOnly => has_entry,
            CachePolicy::StaleWhileRevalidate => has_entry,
            CachePolicy::NetworkFirst | CachePolicy::OnlineOnly => false,
        }
    }

    /// Whether this policy needs background revalidation after serving cache.
    /// Returns true ONLY for StaleWhileRevalidate when an entry exists.
    pub fn needs_revalidation(&self, decision: &RouteDecision, route: &str) -> bool {
        decision.cache_policy == CachePolicy::StaleWhileRevalidate
            && self.get(decision.profile, route).is_some()
    }

    /// Determine if the route can work offline.
    pub fn can_serve_offline(&self, decision: &RouteDecision, route: &str) -> bool {
        let has_entry = self.get(decision.profile, route).is_some();
        match decision.cache_policy {
            CachePolicy::CacheFirst | CachePolicy::LocalOnly
            | CachePolicy::StaleWhileRevalidate => has_entry,
            CachePolicy::NetworkFirst => has_entry,
            CachePolicy::OnlineOnly => false,
        }
    }

    pub fn entry_count(&self) -> usize {
        self.storage.keys().len()
    }

    pub fn clear(&self) {
        for key in self.storage.keys() {
            self.storage.remove(&key);
        }
    }
}

// ── Session connectivity integration ─────────────────────────────────

impl PlatformSession {
    pub fn can_serve_offline_for(
        &self,
        cache: &CacheManager,
        decision: &RouteDecision,
        route: &str,
    ) -> bool {
        if self.is_online() { return false; }
        cache.can_serve_offline(decision, route)
    }
}

// ── Tests ────────────────────────────────────────────────────────────

// NOTE: Tests kept inline because they test CachePolicy enforcement,
// profile scoping, and offline behavior — public API, kept inline
// for convenience alongside the implementation.
#[cfg(test)]
mod tests {
    use super::*;

    fn cache() -> CacheManager { CacheManager::in_memory() }

    fn decision_with(policy: CachePolicy, profile: Profile) -> RouteDecision {
        RouteDecision {
            source: RouteSource::RemoteServer, presentation: Presentation::Morph,
            view_kind: ViewKind::WebView, protocol: ProtocolHint::Default,
            cache_policy: policy, profile, native_view_id: None, target: None,
            capabilities: vec![], auth_origin: None,
        }
    }

    #[test]
    fn store_and_retrieve() {
        let c = cache();
        c.store(Profile::App, "/app/home", b"<html>hello</html>", "text/html");
        let entry = c.get(Profile::App, "/app/home").unwrap();
        assert_eq!(entry.body, b"<html>hello</html>");
        assert_eq!(entry.content_type, "text/html");
    }

    #[test]
    fn profile_scoping_isolates_entries() {
        let c = cache();
        c.store(Profile::App, "/data", b"app-data", "text/plain");
        c.store(Profile::UntrustedRemote, "/data", b"untrusted-data", "text/plain");
        assert_eq!(c.get(Profile::App, "/data").unwrap().body, b"app-data");
        assert_eq!(c.get(Profile::UntrustedRemote, "/data").unwrap().body, b"untrusted-data");
    }

    #[test]
    fn get_missing_returns_none() {
        assert!(cache().get(Profile::App, "/nonexistent").is_none());
    }

    #[test]
    fn invalidate_removes_entry() {
        let c = cache();
        c.store(Profile::App, "/app/home", b"data", "text/plain");
        assert!(c.get(Profile::App, "/app/home").is_some());
        c.invalidate(Profile::App, "/app/home");
        assert!(c.get(Profile::App, "/app/home").is_none());
    }

    #[test]
    fn invalidate_routes_removes_across_profiles() {
        let c = cache();
        c.store(Profile::App, "/shared", b"app", "text/plain");
        c.store(Profile::TrustedRemote, "/shared", b"trusted", "text/plain");
        c.invalidate_routes(&["/shared"]);
        assert!(c.get(Profile::App, "/shared").is_none());
        assert!(c.get(Profile::TrustedRemote, "/shared").is_none());
    }

    #[test]
    fn clear_removes_everything() {
        let c = cache();
        c.store(Profile::App, "/a", b"a", "text/plain");
        c.store(Profile::TrustedRemote, "/b", b"b", "text/plain");
        assert_eq!(c.entry_count(), 2);
        c.clear();
        assert_eq!(c.entry_count(), 0);
    }

    #[test]
    fn stale_while_revalidate_needs_revalidation() {
        let c = cache();
        let d = decision_with(CachePolicy::StaleWhileRevalidate, Profile::App);
        c.store(Profile::App, "/route", b"data", "text/plain");
        assert!(c.needs_revalidation(&d, "/route"));
    }

    #[test]
    fn cache_first_does_not_need_revalidation() {
        let c = cache();
        let d = decision_with(CachePolicy::CacheFirst, Profile::App);
        c.store(Profile::App, "/route", b"data", "text/plain");
        assert!(!c.needs_revalidation(&d, "/route"));
    }
}
