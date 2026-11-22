//! Registration registry: register, replace, ownership, generations, rotation
//! rule (spec 82 feature 00, T3-T6).
//!
//! WHY: the proxy must accept, replace and refuse registrations correctly, and
//! include only backends that pass every check in the rotation rule.
//!
//! WHAT: `BackendRegistry` over a config service `agents` and a simulated clock.
//!
//! HOW: the tests build the generated wire messages directly and drive the
//! registry with the injected resolver and clock. No network is used.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron::SimulatedClock;
use foundation_netio::shared::client::{DnsResolver, StaticSocketAddr};
use foundation_proxy::config::ServiceConfig;
use foundation_proxy::registration::proto::v1::{
    BackendHealth, BackendReport, BackendSpec, DeregisterRequest, Protocol, RegisterRequest,
    RegisterResponse, ReportRequest, ReportResponse, Route, SessionRef,
};
use foundation_proxy::registration::{
    BackendRegistry, PeerIdentity, RegistrationConfig, RegistrationState,
};
use foundation_proxy::router::{Router, RouterHandle};
use foundation_proxy::runtime::ServiceRuntime;
use futures_lite::future::block_on;

const HOST: &str = "agents.example.com";
const SERVICE: &str = "agents";

struct Harness {
    registry: Arc<BackendRegistry>,
    clock: Arc<SimulatedClock>,
}

impl Harness {
    fn new(config: RegistrationConfig) -> Self {
        Self::with_resolver(
            config,
            Arc::new(StaticSocketAddr::new(SocketAddr::from((
                [127, 0, 0, 1],
                9000,
            )))),
        )
    }

    fn with_resolver(config: RegistrationConfig, resolver: Arc<dyn DnsResolver>) -> Self {
        let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
        let ctx = Ctx::fixed(clock.clone());
        let base = vec![Arc::new(ServiceRuntime::new(ServiceConfig::new(
            SERVICE, HOST,
        )))];
        let router = RouterHandle::new(Arc::new(Router::new(base.clone())));
        let registry = Arc::new(BackendRegistry::new(base, router, ctx, config, resolver));
        Self { registry, clock }
    }

    fn advance(&self, by: Duration) {
        self.clock.advance(by);
        self.registry.sweep();
    }
}

fn spec(backend_id: &str, address: &str, port: u16, version: &str) -> BackendSpec {
    BackendSpec {
        backend_id: backend_id.to_string(),
        service: SERVICE.to_string(),
        route: Some(Route {
            host: HOST.to_string(),
            path_prefix: String::new(),
            ..Default::default()
        })
        .into(),
        protocol: Protocol::PROTOCOL_HTTP.into(),
        address: address.to_string(),
        port: u32::from(port),
        weight: 0,
        version: version.to_string(),
        ..Default::default()
    }
}

fn register(
    registry: &BackendRegistry,
    peer: &PeerIdentity,
    session_id: &str,
    host_id: &str,
    specs: Vec<BackendSpec>,
) -> RegisterResponse {
    let req = RegisterRequest {
        session: Some(SessionRef {
            session_id: session_id.to_string(),
            host_id: host_id.to_string(),
            ..Default::default()
        })
        .into(),
        backends: specs,
        ..Default::default()
    };
    block_on(registry.register(peer, &req)).expect("register")
}

fn report(
    registry: &BackendRegistry,
    peer: &PeerIdentity,
    session_id: &str,
    host_id: &str,
    health: BackendHealth,
    ids: &[&str],
) -> ReportResponse {
    let req = ReportRequest {
        session: Some(SessionRef {
            session_id: session_id.to_string(),
            host_id: host_id.to_string(),
            ..Default::default()
        })
        .into(),
        backends: ids
            .iter()
            .map(|id| BackendReport {
                backend_id: (*id).to_string(),
                health: health.into(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    registry.report(peer, &req).expect("report")
}

fn peer(host_id: &str) -> PeerIdentity {
    PeerIdentity::from_certificate(host_id, format!("fingerprint-{host_id}"))
}

fn backend_urls(registry: &BackendRegistry) -> Vec<String> {
    let router = registry.router();
    let Some(service) = router.route(HOST, "/") else {
        return Vec::new();
    };
    service
        .backends()
        .iter()
        .map(|b| b.url().to_string())
        .collect()
}

#[test]
fn register_then_ready_puts_the_backend_in_rotation() {
    let h = Harness::new(RegistrationConfig::new());
    let peer = peer("host-17");
    let response = register(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        vec![spec("host-17/agent", "127.0.0.1", 7443, "1")],
    );
    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].rejected_reason.len(), 0);
    assert_eq!(response.results[0].generation, 1);
    assert!(response.report_interval_ms > 0);
    assert!(response.lease_ttl_ms > 0);

    // Not in rotation until the sidecar reports READY.
    assert_eq!(backend_urls(&h.registry).len(), 0);

    let reported = report(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        BackendHealth::BACKEND_HEALTH_READY,
        &["host-17/agent"],
    );
    assert_eq!(reported.unknown_backend_ids.len(), 0);
    assert_eq!(backend_urls(&h.registry), vec!["http://127.0.0.1:7443"]);
}

#[test]
fn generation_goes_up_only_when_the_spec_or_session_changes() {
    let h = Harness::new(RegistrationConfig::new());
    let peer = peer("host-17");
    let version = |v: &str| {
        register(
            &h.registry,
            &peer,
            "session-1",
            "host-17",
            vec![spec("host-17/agent", "127.0.0.1", 7443, v)],
        )
        .results[0]
            .generation
    };
    assert_eq!(version("1"), 1, "first registration");
    assert_eq!(version("1"), 1, "the same spec keeps the generation");
    assert_eq!(version("2"), 2, "a changed version goes up by one");
}

#[test]
fn a_live_owner_refuses_another_sidecars_registration() {
    let h = Harness::new(RegistrationConfig::new());
    let a = peer("host-a");
    let b = peer("host-b");
    let first = register(
        &h.registry,
        &a,
        "session-a",
        "host-a",
        vec![spec("shared/agent", "127.0.0.1", 7443, "1")],
    );
    assert_eq!(first.results[0].rejected_reason.len(), 0);

    let second = register(
        &h.registry,
        &b,
        "session-b",
        "host-b",
        vec![spec("shared/agent", "127.0.0.1", 7444, "1")],
    );
    assert!(
        second.results[0]
            .rejected_reason
            .contains("belongs to sidecar"),
        "got {:?}",
        second.results[0].rejected_reason
    );
    assert_eq!(h.registry.snapshot()[0].owner_host_id, "host-a");
}

#[test]
fn a_new_session_of_the_same_host_replaces_the_old_one() {
    let h = Harness::new(RegistrationConfig::new());
    let peer = peer("host-17");
    register(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        vec![spec("host-17/agent", "127.0.0.1", 7443, "1")],
    );
    let runtime_before = Arc::clone(&h.registry.snapshot()[0].runtime);

    let second = register(
        &h.registry,
        &peer,
        "session-2",
        "host-17",
        vec![spec("host-17/agent", "127.0.0.1", 7443, "1")],
    );
    assert_eq!(second.results[0].generation, 2, "generation goes up");
    let runtime_after = Arc::clone(&h.registry.snapshot()[0].runtime);
    assert!(
        Arc::ptr_eq(&runtime_before, &runtime_after),
        "an unchanged address keeps its runtime"
    );
    assert_eq!(h.registry.snapshot()[0].session_id, "session-2");

    let old = registry_report(&h.registry, &peer, "session-1", "host-17");
    assert!(
        matches!(
            old,
            foundation_proxy::registration::RegistrationError::SessionSuperseded { .. }
        ),
        "a report from the replaced session is refused"
    );
}

fn registry_report(
    registry: &BackendRegistry,
    peer: &PeerIdentity,
    session_id: &str,
    host_id: &str,
) -> foundation_proxy::registration::RegistrationError {
    let req = ReportRequest {
        session: Some(SessionRef {
            session_id: session_id.to_string(),
            host_id: host_id.to_string(),
            ..Default::default()
        })
        .into(),
        backends: Vec::new(),
        ..Default::default()
    };
    registry.report(peer, &req).expect_err("superseded")
}

#[test]
fn too_many_backends_is_refused() {
    let h = Harness::new(RegistrationConfig::new().with_max_backends_per_session(1));
    let peer = peer("host-17");
    let req = RegisterRequest {
        session: Some(SessionRef {
            session_id: "session-1".to_string(),
            host_id: "host-17".to_string(),
            ..Default::default()
        })
        .into(),
        backends: vec![
            spec("host-17/a", "127.0.0.1", 7001, "1"),
            spec("host-17/b", "127.0.0.1", 7002, "1"),
        ],
        ..Default::default()
    };
    let error = block_on(h.registry.register(&peer, &req)).expect_err("limit");
    assert!(matches!(
        error,
        foundation_proxy::registration::RegistrationError::TooManyBackends { count: 2, limit: 1 }
    ));
}

#[test]
fn the_operator_drain_state_excludes_a_ready_backend() {
    let h = Harness::new(RegistrationConfig::new());
    let peer = peer("host-17");
    register(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        vec![spec("host-17/agent", "127.0.0.1", 7443, "1")],
    );
    report(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        BackendHealth::BACKEND_HEALTH_READY,
        &["host-17/agent"],
    );
    let snapshot = h.registry.snapshot();
    let runtime = Arc::clone(&snapshot[0].runtime);

    runtime.set_state(foundation_proxy::config::BackendState::Draining);
    let router = h.registry.router();
    let service = router.route(HOST, "/").expect("service");
    assert!(
        service.pick().is_none(),
        "a drained backend gets no new request"
    );

    runtime.set_state(foundation_proxy::config::BackendState::Active);
    assert!(service.pick().is_some(), "an active backend gets one");
}

#[test]
fn the_proxy_probe_must_also_pass() {
    let h = Harness::new(RegistrationConfig::new());
    let peer = peer("host-17");
    register(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        vec![spec("host-17/agent", "127.0.0.1", 7443, "1")],
    );
    report(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        BackendHealth::BACKEND_HEALTH_READY,
        &["host-17/agent"],
    );
    let runtime = Arc::clone(&h.registry.snapshot()[0].runtime);
    let router = h.registry.router();
    let service = router.route(HOST, "/").expect("service");

    runtime.set_healthy(false);
    assert!(
        service.pick().is_none(),
        "the sidecar says READY but the proxy probe fails"
    );
    runtime.set_healthy(true);
    assert!(service.pick().is_some());
}

#[test]
fn a_not_ready_backend_is_not_in_the_router() {
    let h = Harness::new(RegistrationConfig::new());
    let peer = peer("host-17");
    register(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        vec![spec("host-17/agent", "127.0.0.1", 7443, "1")],
    );
    report(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        BackendHealth::BACKEND_HEALTH_NOT_READY,
        &["host-17/agent"],
    );
    assert_eq!(backend_urls(&h.registry).len(), 0);
}

#[test]
fn an_address_outside_the_policy_is_rejected_alone() {
    let h = Harness::new(RegistrationConfig::new());
    let peer = peer("host-17");
    let response = register(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        vec![
            spec("host-17/bad", "8.8.8.8", 7443, "1"),
            spec("host-17/good", "127.0.0.1", 7443, "1"),
        ],
    );
    assert!(response.results[0]
        .rejected_reason
        .contains("outside allowed_networks"));
    assert_eq!(response.results[1].rejected_reason.len(), 0);
    assert_eq!(h.registry.snapshot().len(), 1);
}

#[test]
fn a_dns_name_that_resolves_outside_the_policy_is_rejected() {
    let resolver = Arc::new(StaticSocketAddr::new(SocketAddr::from(([8, 8, 8, 8], 443))));
    let h = Harness::with_resolver(RegistrationConfig::new(), resolver);
    let peer = peer("host-17");
    let response = register(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        vec![spec("host-17/agent", "backend.example", 7443, "1")],
    );
    assert!(response.results[0]
        .rejected_reason
        .contains("outside allowed_networks"));
}

#[test]
fn deregister_moves_the_backend_out_of_rotation() {
    let h = Harness::new(RegistrationConfig::new());
    let peer = peer("host-17");
    register(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        vec![spec("host-17/agent", "127.0.0.1", 7443, "1")],
    );
    report(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        BackendHealth::BACKEND_HEALTH_READY,
        &["host-17/agent"],
    );
    assert_eq!(backend_urls(&h.registry).len(), 1);

    let req = DeregisterRequest {
        session: Some(SessionRef {
            session_id: "session-1".to_string(),
            host_id: "host-17".to_string(),
            ..Default::default()
        })
        .into(),
        backend_ids: vec!["host-17/agent".to_string()],
        ..Default::default()
    };
    let response = h.registry.deregister(&peer, &req).expect("deregister");
    assert_eq!(response.draining, vec!["host-17/agent"]);
    assert_eq!(backend_urls(&h.registry).len(), 0);
    assert_eq!(h.registry.snapshot().len(), 0);
}

#[test]
fn a_stale_entry_revives_on_the_next_report() {
    let h = Harness::new(RegistrationConfig::new());
    let peer = peer("host-17");
    register(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        vec![spec("host-17/agent", "127.0.0.1", 7443, "1")],
    );
    report(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        BackendHealth::BACKEND_HEALTH_READY,
        &["host-17/agent"],
    );

    h.advance(Duration::from_secs(31));
    assert!(
        matches!(
            h.registry.snapshot()[0].state,
            RegistrationState::Stale { .. }
        ),
        "the lease ended"
    );
    assert_eq!(backend_urls(&h.registry).len(), 0);

    report(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        BackendHealth::BACKEND_HEALTH_READY,
        &["host-17/agent"],
    );
    assert!(matches!(
        h.registry.snapshot()[0].state,
        RegistrationState::Live
    ));
    assert_eq!(backend_urls(&h.registry).len(), 1);
}
