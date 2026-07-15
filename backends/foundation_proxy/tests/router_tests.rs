//! Router matching: exact host, wildcard host, longest path prefix, unknown 404.

use std::sync::Arc;

use foundation_proxy::config::ServiceConfig;
use foundation_proxy::runtime::ServiceRuntime;
use foundation_proxy::router::Router;

fn service(name: &str, host: &str, prefix: Option<&str>) -> Arc<ServiceRuntime> {
    let mut cfg = ServiceConfig::new(name, host).backend("http://127.0.0.1:1");
    if let Some(p) = prefix {
        cfg = cfg.path_prefix(p);
    }
    Arc::new(ServiceRuntime::new(cfg))
}

/// WHY: The primary dispatch key is the Host header.
/// WHAT: Each host routes to its own service; the port on the Host is ignored.
#[test]
fn test_exact_host_routing() {
    let router = Router::new(vec![
        service("a", "a.example.com", None),
        service("b", "b.example.com", None),
    ]);
    assert_eq!(router.route("a.example.com", "/").unwrap().config().name, "a");
    assert_eq!(
        router.route("b.example.com:8080", "/x").unwrap().config().name,
        "b",
        "port on Host must be ignored"
    );
}

/// WHY: An unknown host is a 404, not a panic (`None` → handler answers 404).
/// WHAT: A host with no matching service returns `None`.
#[test]
fn test_unknown_host_returns_none() {
    let router = Router::new(vec![service("a", "a.example.com", None)]);
    assert!(router.route("nope.example.com", "/").is_none());
}

/// WHY: Wildcard hosts let a subdomain family share one service, but an exact
/// host must still win when both match.
/// WHAT: `*.example.com` matches any subdomain; an exact host beats the wildcard.
#[test]
fn test_wildcard_host_and_exact_precedence() {
    let router = Router::new(vec![
        service("wild", "*.example.com", None),
        service("exact", "app.example.com", None),
    ]);
    assert_eq!(
        router.route("foo.example.com", "/").unwrap().config().name,
        "wild",
        "arbitrary subdomain matches the wildcard"
    );
    assert_eq!(
        router.route("deep.foo.example.com", "/").unwrap().config().name,
        "wild",
        "deep subdomain still matches the wildcard"
    );
    assert_eq!(
        router.route("app.example.com", "/").unwrap().config().name,
        "exact",
        "exact host must beat the wildcard"
    );
    assert!(
        router.route("example.com", "/").is_none(),
        "wildcard must not match the bare apex"
    );
}

/// WHY: Under one host, the longest matching path prefix wins.
/// WHAT: `/api/v1/x` picks `/api/v1` over `/api` over root; a non-prefix on a
/// boundary (`/apix`) does not match `/api`.
#[test]
fn test_longest_path_prefix_wins() {
    let router = Router::new(vec![
        service("root", "app.local", None),
        service("api", "app.local", Some("/api")),
        service("apiv1", "app.local", Some("/api/v1")),
    ]);
    assert_eq!(router.route("app.local", "/api/v1/x").unwrap().config().name, "apiv1");
    assert_eq!(router.route("app.local", "/api/thing").unwrap().config().name, "api");
    assert_eq!(router.route("app.local", "/other").unwrap().config().name, "root");
    assert_eq!(
        router.route("app.local", "/apix").unwrap().config().name,
        "root",
        "prefix must match on a path-segment boundary, not substring"
    );
}

/// WHY: Query strings must not defeat prefix matching.
/// WHAT: `/api?x=1` still routes to the `/api` service.
#[test]
fn test_query_string_ignored_for_prefix() {
    let router = Router::new(vec![
        service("root", "app.local", None),
        service("api", "app.local", Some("/api")),
    ]);
    assert_eq!(router.route("app.local", "/api?x=1").unwrap().config().name, "api");
}
