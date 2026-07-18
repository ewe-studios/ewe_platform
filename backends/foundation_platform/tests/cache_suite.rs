//! Tests for cache tiers — per-route cache with foundation_db StorageProvider backend.

use foundation_platform::*;

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
