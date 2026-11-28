//! Tests for the router catch-all.

use foundation_http::shared::router::Router;
use foundation_netio::shared::http::SimpleMethod;

#[test]
fn given_a_router_with_a_fallback_when_no_route_matches_then_the_fallback_runs() {
    let mut router = Router::<u32>::new();
    router.set_fallback(7);

    assert_eq!(
        router
            .dispatch(&SimpleMethod::GET, "/events/abc")
            .expect("fallback"),
        7
    );
    assert_eq!(
        router
            .dispatch(&SimpleMethod::POST, "/a/deep/path")
            .expect("fallback"),
        7
    );
}

#[test]
fn given_a_router_without_a_fallback_when_no_route_matches_then_it_misses() {
    let router = Router::<u32>::new();
    assert!(router.dispatch(&SimpleMethod::GET, "/events/abc").is_none());
}
