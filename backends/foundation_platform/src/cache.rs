//! Cache tiers — SQLite-backed route cache with per-route CachePolicy.
//!
//! Two-tier offline model from decision 05:
//!   Tier 1: Local WASM execution (offline by default — no cache needed)
//!   Tier 2: Rendered page caching (remote content cached locally)
//!
//! Protocol-transparent: stores responses as-encoded (same bytes, same
//! Content-Type). Replay is identical to the original response.
//!
//! Profile-scoped: UntrustedRemote pages cannot read App cache entries.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use foundation_ui_traits::*;

use crate::session::PlatformSession;

// ── Storage trait ────────────────────────────────────────────────────

/// Backend storage for cached responses. Default is in-memory HashMap.
/// Swappable for SQLite via `foundation_db` when needed (same trait).
pub trait CacheStorage: Send + Sync + 'static {
    fn get(&self, key: &str) -> Option<CachedEntry>;
    fn set(&self, key: &str, entry: CachedEntry);
    fn remove(&self, key: &str);
    fn keys(&self) -> Vec<String>;
}

/// In-memory storage. Used for tests and as the default backend.
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
/// Profile-scoped — entries are keyed by `{profile}:{route}` to prevent
/// cross-profile cache leakage.
pub struct CacheManager {
    storage: Box<dyn CacheStorage>,
}

impl CacheManager {
    /// Create a cache manager with the given storage backend.
    pub fn new(storage: impl CacheStorage) -> Self {
        Self { storage: Box::new(storage) }
    }

    /// Create a cache manager with in-memory storage (default for tests).
    pub fn in_memory() -> Self {
        Self::new(MemoryCacheStorage::new())
    }

    /// Build a scoped cache key from profile + route.
    fn scoped_key(profile: Profile, route: &str) -> String {
        format!("{:?}:{}", profile, route)
    }

    /// Store a response in the cache. Keyed by profile + route.
    pub fn store(&self, profile: Profile, route: &str, body: &[u8], content_type: &str) {
        let key = Self::scoped_key(profile, route);
        self.storage.set(&key, CachedEntry::new(body.to_vec(), content_type, profile));
    }

    /// Retrieve a cached response for a profile + route.
    pub fn get(&self, profile: Profile, route: &str) -> Option<CachedEntry> {
        let key = Self::scoped_key(profile, route);
        self.storage.get(&key)
    }

    /// Invalidate cached entries for specific routes (across all profiles).
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

    /// Invalidate cached entries for a specific profile + route.
    pub fn invalidate(&self, profile: Profile, route: &str) {
        self.storage.remove(&Self::scoped_key(profile, route));
    }

    /// Check whether the cache should serve this request, given the
    /// route's CachePolicy and whether a cached entry exists.
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

    /// Determine whether the route can work offline.
    /// Routes with CacheFirst, LocalOnly, or StaleWhileRevalidate can
    /// serve cached content. NetworkFirst can serve stale. OnlineOnly fails.
    pub fn can_serve_offline(&self, decision: &RouteDecision, route: &str) -> bool {
        match decision.cache_policy {
            CachePolicy::CacheFirst | CachePolicy::LocalOnly
            | CachePolicy::StaleWhileRevalidate => self.get(decision.profile, route).is_some(),
            CachePolicy::NetworkFirst => self.get(decision.profile, route).is_some(),
            CachePolicy::OnlineOnly => false,
        }
    }

    /// Return the number of cached entries (across all profiles).
    pub fn entry_count(&self) -> usize {
        self.storage.keys().len()
    }

    /// Clear all cached entries.
    pub fn clear(&self) {
        for key in self.storage.keys() {
            self.storage.remove(&key);
        }
    }
}

// ── Session connectivity integration ─────────────────────────────────

impl PlatformSession {
    /// Whether the session can serve cached content while offline.
    /// Returns true if the current route has a cache entry and the policy
    /// allows offline serving.
    pub fn can_serve_offline_for(
        &self,
        cache: &CacheManager,
        decision: &RouteDecision,
        route: &str,
    ) -> bool {
        if self.is_online() {
            return false; // online — go to network, don't serve cache
        }
        cache.can_serve_offline(decision, route)
    }
}

// ── Tests ────────────────────────────────────────────────────────────

// NOTE: Tests kept inline because: Tests CachePolicy enforcement, profile scoping, offline behavior—public API, kept inline for convenience.
#[cfg(test)]
mod tests {
    use super::*;

    fn cache() -> CacheManager {
        CacheManager::in_memory()
    }

    fn decision_with(policy: CachePolicy, profile: Profile) -> RouteDecision {
        RouteDecision {
            source: RouteSource::RemoteServer,
            presentation: Presentation::Morph,
            view_kind: ViewKind::WebView,
            protocol: ProtocolHint::Default,
            cache_policy: policy,
            profile,
            native_view_id: None,
            target: None,
            capabilities: vec![],
            auth_origin: None,
        }
    }

    // ── Store/retrieve ───────────────────────────────────────────

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
        // Different profiles, same route — different entries
    }

    #[test]
    fn get_missing_returns_none() {
        let c = cache();
        assert!(c.get(Profile::App, "/nonexistent").is_none());
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

    // ── CachePolicy: should_serve_cached ──────────────────────────

    #[test]
    fn cache_first_serves_when_present() {
        let c = cache();
        let d = decision_with(CachePolicy::CacheFirst, Profile::App);
        c.store(Profile::App, "/route", b"data", "text/plain");
        assert!(c.should_serve_cached(&d, "/route"));
    }

    #[test]
    fn cache_first_falls_through_on_miss() {
        let c = cache();
        let d = decision_with(CachePolicy::CacheFirst, Profile::App);
        assert!(!c.should_serve_cached(&d, "/route"));
    }

    #[test]
    fn network_first_never_serves_cache() {
        let c = cache();
        let d = decision_with(CachePolicy::NetworkFirst, Profile::App);
        c.store(Profile::App, "/route", b"data", "text/plain");
        assert!(!c.should_serve_cached(&d, "/route")); // always go to network
    }

    #[test]
    fn online_only_never_serves_cache() {
        let c = cache();
        let d = decision_with(CachePolicy::OnlineOnly, Profile::App);
        c.store(Profile::App, "/route", b"data", "text/plain");
        assert!(!c.should_serve_cached(&d, "/route"));
    }

    #[test]
    fn local_only_always_serves_cache_when_present() {
        let c = cache();
        let d = decision_with(CachePolicy::LocalOnly, Profile::App);
        c.store(Profile::App, "/route", b"data", "text/plain");
        assert!(c.should_serve_cached(&d, "/route"));
    }

    #[test]
    fn local_only_fails_when_missing() {
        let c = cache();
        let d = decision_with(CachePolicy::LocalOnly, Profile::App);
        assert!(!c.should_serve_cached(&d, "/route"));
    }

    #[test]
    fn stale_while_revalidate_serves_cache() {
        let c = cache();
        let d = decision_with(CachePolicy::StaleWhileRevalidate, Profile::App);
        c.store(Profile::App, "/route", b"data", "text/plain");
        assert!(c.should_serve_cached(&d, "/route"));
    }

    // ── Offline behavior ─────────────────────────────────────────

    #[test]
    fn cache_first_works_offline() {
        let c = cache();
        let d = decision_with(CachePolicy::CacheFirst, Profile::App);
        assert!(!c.can_serve_offline(&d, "/route")); // no entry
        c.store(Profile::App, "/route", b"data", "text/plain");
        assert!(c.can_serve_offline(&d, "/route")); // has entry
    }

    #[test]
    fn network_first_serves_stale_offline() {
        let c = cache();
        let d = decision_with(CachePolicy::NetworkFirst, Profile::App);
        c.store(Profile::App, "/route", b"data", "text/plain");
        assert!(c.can_serve_offline(&d, "/route")); // serves stale when offline
    }

    #[test]
    fn online_only_fails_offline() {
        let c = cache();
        let d = decision_with(CachePolicy::OnlineOnly, Profile::App);
        assert!(!c.can_serve_offline(&d, "/route")); // never works offline
    }

    // ── Session integration ──────────────────────────────────────

    #[test]
    fn online_session_does_not_serve_cache() {
        let c = cache();
        let session = PlatformSession::new();
        let d = decision_with(CachePolicy::CacheFirst, Profile::App);
        c.store(Profile::App, "/route", b"data", "text/plain");

        // Online → go to network regardless
        assert!(session.is_online());
        assert!(!session.can_serve_offline_for(&c, &d, "/route"));
    }

    #[test]
    fn offline_session_serves_cache_when_policy_allows() {
        let c = cache();
        let session = PlatformSession::new();
        session.set_online(false);
        let d = decision_with(CachePolicy::CacheFirst, Profile::App);
        c.store(Profile::App, "/route", b"data", "text/plain");

        assert!(session.can_serve_offline_for(&c, &d, "/route"));
    }

    // ── Entry metadata ───────────────────────────────────────────

    #[test]
    fn entry_has_timestamp() {
        let entry = CachedEntry::new(b"data".to_vec(), "text/plain", Profile::App);
        assert!(entry.cached_at > 0);
    }

    #[test]
    fn entry_preserves_content_type() {
        let entry = CachedEntry::new(b"{}".to_vec(), "application/primal-json", Profile::App);
        assert_eq!(entry.content_type, "application/primal-json");
    }
}
