//! Tests for URL path pattern matching with `*` and `**` globs.

use foundation_platform::pattern::{extract_path, Pattern, PatternRouter};
use foundation_platform::*;

// ── Pattern construction ──────────────────────────────────────

#[test]
fn empty_pattern() {
    let p = Pattern::new("").unwrap();
    assert!(p.matches(""));
    assert!(p.matches("/"));
    assert!(!p.matches("/anything"));
}

#[test]
fn root_pattern() {
    let p = Pattern::new("/").unwrap();
    assert!(p.matches(""));
    assert!(p.matches("/"));
    assert!(!p.matches("/app"));
}

#[test]
fn exact_match() {
    let p = Pattern::new("/app/items").unwrap();
    assert!(p.matches("/app/items"));
    assert!(p.matches("app/items"));
    assert!(!p.matches("/app/other"));
    assert!(!p.matches("/app/items/detail"));
}

#[test]
fn single_segment() {
    let p = Pattern::new("/app").unwrap();
    assert!(p.matches("/app"));
    assert!(!p.matches("/app/items"));
}

// ── Wildcards ─────────────────────────────────────────────────

#[test]
fn single_wildcard_matches_one_segment() {
    let p = Pattern::new("/app/*/detail").unwrap();
    assert!(p.matches("/app/items/detail"));
    assert!(p.matches("/app/42/detail"));
    assert!(p.matches("/app/foo-bar/detail"));
    assert!(!p.matches("/app/items")); // missing /detail
    assert!(!p.matches("/app/items/sub/detail")); // * = one segment, not two
    assert!(!p.matches("/other/items/detail")); // /app doesn't match
}

#[test]
fn multiple_single_wildcards() {
    let p = Pattern::new("/*/*/detail").unwrap();
    assert!(p.matches("/foo/bar/detail"));
    assert!(!p.matches("/foo/detail")); // only one wildcard-matched segment
    assert!(!p.matches("/foo/bar/baz/detail")); // three segments before detail
}

#[test]
fn double_wildcard_matches_zero_or_more() {
    let p = Pattern::new("/static/**").unwrap();
    assert!(p.matches("/static")); // ** matches zero segments
    assert!(p.matches("/static/css/main.css"));
    assert!(p.matches("/static/js/vendor/lib.js"));
    assert!(p.matches("/static/a/b/c/d/e"));
    assert!(!p.matches("/other/css/main.css")); // /static doesn't match
}

#[test]
fn double_wildcard_with_prefix() {
    let p = Pattern::new("/app/**").unwrap();
    assert!(p.matches("/app"));
    assert!(p.matches("/app/items"));
    assert!(p.matches("/app/items/detail"));
    assert!(!p.matches("/api/items")); // /app doesn't match
}

#[test]
fn trailing_wildcard() {
    let p = Pattern::new("/app/*").unwrap();
    assert!(p.matches("/app/items"));
    assert!(p.matches("/app/dashboard"));
    assert!(!p.matches("/app/items/detail")); // * only matches one segment
}

// ── Pattern validation ────────────────────────────────────────

#[test]
fn double_wildcard_must_be_last() {
    assert!(Pattern::new("/app/**/detail").is_err());
    assert!(Pattern::new("/**/app").is_err());
}

#[test]
fn only_one_double_wildcard() {
    assert!(Pattern::new("/app/**/sub/**").is_err());
}

#[test]
fn partial_wildcards_rejected() {
    assert!(Pattern::new("/app/file*").is_err());
    assert!(Pattern::new("/app/*.html").is_err());
    assert!(Pattern::new("/app/file**.txt").is_err());
}

// ── Pattern display ───────────────────────────────────────────

#[test]
fn display_roundtrips() {
    for pat in &["/app/*/detail", "/static/**", "/app/items", "/auth/*"] {
        let p = Pattern::new(pat).unwrap();
        assert_eq!(p.to_string(), *pat);
    }
}

// ── URL extraction ────────────────────────────────────────────

#[test]
fn extract_path_from_ewe_url() {
    assert_eq!(extract_path("ewe://localhost/app/items?proto=arrow"), "/app/items");
    assert_eq!(extract_path("ewe://localhost/remote/dashboard"), "/remote/dashboard");
    assert_eq!(extract_path("ewe://localhost/"), "/");
}

#[test]
fn extract_path_from_https_url() {
    assert_eq!(extract_path("https://example.com/path"), "/path");
    assert_eq!(extract_path("https://example.com/path?a=1&b=2"), "/path");
}

#[test]
fn extract_path_bare() {
    assert_eq!(extract_path("/app/items"), "/app/items");
    assert_eq!(extract_path("/app/items?proto=arrow"), "/app/items");
}

// ── PatternRouter ─────────────────────────────────────────────

fn intent(path: &str) -> NavigationIntent {
    NavigationIntent {
        url: format!("ewe://localhost{path}"),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    }
}

#[test]
fn router_first_match_wins() {
    let mut router = PatternRouter::new();
    router.route("/app/*", webview_app());
    router.route("/app/special", remote_fetch());

    // Both patterns match "/app/special" - first registered wins
    let d = router.resolve_intent(&intent("/app/special")).unwrap();
    assert_eq!(d.source, RouteSource::WebviewApp);
}

#[test]
fn router_no_match_returns_none() {
    let mut router = PatternRouter::new();
    router.route("/app/*", webview_app());

    let d = router.resolve_intent(&intent("/remote/dashboard"));
    assert!(d.is_none());
}

#[test]
fn router_multiple_patterns() {
    let mut router = PatternRouter::new();
    router.route("/app/*", webview_app());
    router.route("/remote/*", remote_fetch());
    router.route("/cached/*", remote_fetch()
        .with_cache_policy(CachePolicy::CacheFirst));

    assert_eq!(
        router.resolve_intent(&intent("/app/items")).unwrap().source,
        RouteSource::WebviewApp
    );
    assert_eq!(
        router.resolve_intent(&intent("/remote/dashboard")).unwrap().source,
        RouteSource::RemoteServer
    );
    assert_eq!(
        router.resolve_intent(&intent("/cached/data")).unwrap().cache_policy,
        CachePolicy::CacheFirst
    );
}

#[test]
fn router_as_route_handler() {
    use foundation_platform::RouteHandler;

    let mut router = PatternRouter::new();
    router.route("/app/*", webview_app());

    let session = PlatformSession::new_test(std::path::PathBuf::from("."));
    let d = router.resolve(&intent("/app/dashboard"), &session).unwrap();
    assert_eq!(d.source, RouteSource::WebviewApp);
}
