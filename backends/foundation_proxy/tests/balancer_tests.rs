//! Load balancer: weighted round-robin, eligibility gating, max_connections,
//! and the in-flight refcount.

use foundation_proxy::config::{BackendState, BackendTarget, ServiceConfig};
use foundation_proxy::runtime::ServiceRuntime;

fn service_with(backends: Vec<BackendTarget>) -> ServiceRuntime {
    let mut cfg = ServiceConfig::new("svc", "svc.local");
    cfg.backends = backends;
    ServiceRuntime::new(cfg)
}

/// WHY: Weighted round-robin must distribute in proportion to weight.
/// WHAT: Backends weighted 3:1 receive 6:2 over 8 picks (leases held so no
/// capacity is freed mid-run).
#[test]
fn test_weighted_round_robin_distribution() {
    let svc = service_with(vec![
        BackendTarget::new("http://a").with_weight(3),
        BackendTarget::new("http://b").with_weight(1),
    ]);

    let mut count_a = 0;
    let mut count_b = 0;
    let mut leases = Vec::new();
    for _ in 0..8 {
        let lease = svc.pick().expect("a backend is eligible");
        match lease.backend().url() {
            "http://a" => count_a += 1,
            "http://b" => count_b += 1,
            other => panic!("unexpected backend {other}"),
        }
        leases.push(lease);
    }
    assert_eq!(count_a, 6, "weight-3 backend should get 6/8");
    assert_eq!(count_b, 2, "weight-1 backend should get 2/8");
}

/// WHY: Only `Active`, healthy backends receive traffic.
/// WHAT: An unhealthy or paused backend is skipped; a lone eligible backend
/// always wins.
#[test]
fn test_eligibility_excludes_unhealthy_and_paused() {
    let svc = service_with(vec![
        BackendTarget::new("http://a"),
        BackendTarget::new("http://b"),
    ]);
    let backends = svc.backends();

    // Mark A unhealthy → every pick is B.
    backends[0].set_healthy(false);
    for _ in 0..4 {
        assert_eq!(svc.pick().unwrap().backend().url(), "http://b");
    }

    // Pause B too → nothing is eligible.
    backends[1].set_state(BackendState::Paused);
    assert!(svc.pick().is_none(), "no eligible backend → None (handler 503s)");

    // Recover A → picks resume.
    backends[0].set_healthy(true);
    assert_eq!(svc.pick().unwrap().backend().url(), "http://a");
}

/// WHY: `max_connections` caps concurrent in-flight requests per backend.
/// WHAT: A cap of 1 allows one lease; a second pick spills to the other backend
/// and, when both are saturated, returns `None`.
#[test]
fn test_max_connections_enforced() {
    let svc = service_with(vec![
        BackendTarget::new("http://a").with_max_connections(1),
        BackendTarget::new("http://b").with_max_connections(1),
    ]);

    let l1 = svc.pick().expect("first pick");
    let l2 = svc.pick().expect("second pick spills to the other backend");
    assert_ne!(
        l1.backend().url(),
        l2.backend().url(),
        "each backend allows only one in-flight"
    );
    assert!(svc.pick().is_none(), "both at capacity → None");

    // Releasing one lease frees exactly one slot.
    let freed = l1.backend().url().to_string();
    drop(l1);
    let l3 = svc.pick().expect("a slot freed up");
    assert_eq!(l3.backend().url(), freed);
    drop((l2, l3));
}

/// WHY: The in-flight refcount (used for draining in stage 3) must track leases.
/// WHAT: Holding a lease raises `inflight` to 1; dropping it returns to 0.
#[test]
fn test_inflight_refcount_tracks_leases() {
    let svc = service_with(vec![BackendTarget::new("http://a")]);
    assert_eq!(svc.backends()[0].inflight(), 0);
    let lease = svc.pick().unwrap();
    assert_eq!(svc.backends()[0].inflight(), 1, "lease increments in-flight");
    drop(lease);
    assert_eq!(svc.backends()[0].inflight(), 0, "drop decrements in-flight");
}
