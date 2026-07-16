//! Integration tests for ServiceRuntime load balancing — pick() and pick_sticky().

use std::sync::Arc;
use std::time::Duration;

use foundation_proxy::config::{BackendTarget, ServiceConfig};
use foundation_proxy::runtime::{ServiceRuntime, STICKY_COOKIE};

fn backend(url: &str, weight: u32) -> BackendTarget {
    BackendTarget {
        url: url.to_string(),
        weight,
        max_connections: 100,
    }
}

fn service(name: &str, backends: Vec<BackendTarget>) -> ServiceRuntime {
    ServiceRuntime::new(ServiceConfig {
        name: name.to_string(),
        host: "test.local".to_string(),
        path_prefix: Some("/".to_string()),
        backends,
        health_check: None,
    })
}

#[test]
fn sticky_pick_routes_to_preferred_backend() {
    let svc = service("test", vec![
        backend("http://a:8080", 1),
        backend("http://b:8081", 1),
    ]);
    for _ in 0..10 {
        let (lease, idx) = svc.pick_sticky(Some(1)).expect("lease");
        assert_eq!(idx, 1);
        assert_eq!(lease.backend().url(), "http://b:8081");
    }
}

#[test]
fn sticky_falls_back_when_preferred_ineligible() {
    let svc = service("test", vec![
        backend("http://a:8080", 1),
        backend("http://b:8081", 1),
    ]);
    svc.backends()[1].set_healthy(false);
    let (lease, idx) = svc.pick_sticky(Some(1)).expect("lease");
    assert_eq!(idx, 0, "should fall back to healthy backend");
    assert_eq!(lease.backend().url(), "http://a:8080");
}

#[test]
fn single_backend_returns_idx_zero() {
    let svc = service("test", vec![backend("http://only:8080", 1)]);
    let (lease, idx) = svc.pick_sticky(None).expect("lease");
    assert_eq!(idx, 0);
    assert_eq!(lease.backend().url(), "http://only:8080");
}

#[test]
fn pick_round_robins_across_backends() {
    let svc = service("test", vec![
        backend("http://a:8080", 1),
        backend("http://b:8081", 1),
        backend("http://c:8082", 1),
    ]);
    let mut counts = vec![0usize; 3];
    for _ in 0..30 {
        let lease = svc.pick().expect("lease");
        let url = lease.backend().url().to_string();
        if url == "http://a:8080" { counts[0] += 1; }
        else if url == "http://b:8081" { counts[1] += 1; }
        else if url == "http://c:8082" { counts[2] += 1; }
    }
    // With smooth WRR and equal weights, each should get ~10 (±5)
    assert!(counts[0] >= 5, "backend A should get some traffic: got {}", counts[0]);
    assert!(counts[1] >= 5, "backend B should get some traffic: got {}", counts[1]);
    assert!(counts[2] >= 5, "backend C should get some traffic: got {}", counts[2]);
}

#[test]
fn unhealthy_backend_is_not_picked() {
    let svc = service("test", vec![
        backend("http://a:8080", 1),
    ]);
    svc.backends()[0].set_healthy(false);
    assert!(svc.pick().is_none(), "unhealthy backend should not be picked");
}

#[test]
fn paused_backend_is_not_picked() {
    use foundation_proxy::config::BackendState;
    let svc = service("test", vec![
        backend("http://a:8080", 1),
    ]);
    svc.backends()[0].set_state(BackendState::Paused);
    assert!(svc.pick().is_none(), "paused backend should not be picked");
}
