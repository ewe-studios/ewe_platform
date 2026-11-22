//! Registration failure paths: one test for each registry-level row of spec 82
//! feature 00 section 7, with a simulated clock.
//!
//! WHY: every failure path must have a defined result (section 7). These tests
//! drive the registry without a network so the lease timing is exact.
//!
//! WHAT: rows 7.1 to 7.9, 7.12 and 7.13. Rows 7.10 (certificate) and 7.11
//! (enrollment token) belong to the authenticated listener and are proven in
//! `registration_auth_tests`.
//!
//! HOW: a `SimulatedClock` moves the lease past its deadline; the resolver is a
//! static loopback address.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron::SimulatedClock;
use foundation_netio::shared::client::{DnsResolver, StaticSocketAddr};
use foundation_proxy::config::ServiceConfig;
use foundation_proxy::registration::proto::v1::{
    BackendHealth, BackendReport, BackendSpec, Protocol, RegisterRequest, RegisterResponse,
    ReportAction, ReportRequest, ReportResponse, Route, SessionRef,
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
    config: RegistrationConfig,
}

impl Harness {
    fn new(config: RegistrationConfig) -> Self {
        let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
        let ctx = Ctx::fixed(clock.clone());
        let base = vec![Arc::new(ServiceRuntime::new(ServiceConfig::new(
            SERVICE, HOST,
        )))];
        let router = RouterHandle::new(Arc::new(Router::new(base.clone())));
        let registry = Arc::new(BackendRegistry::new(
            base,
            router,
            ctx,
            config.clone(),
            Arc::new(StaticSocketAddr::new(SocketAddr::from((
                [127, 0, 0, 1],
                9000,
            )))) as Arc<dyn DnsResolver>,
        ));
        Self {
            registry,
            clock,
            config,
        }
    }

    fn advance_and_sweep(&self, by: Duration) {
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

fn register_ready(h: &Harness, id: &str, address: &str, port: u16) {
    let peer = peer("host-17");
    register(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        vec![spec(id, address, port, "1")],
    );
    report(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        BackendHealth::BACKEND_HEALTH_READY,
        &[id],
    );
}

/// 7.1 Missed reports: the lease ends, the backend leaves rotation, a report
/// brings it back, and after `stale_remove_after` a report lists it as unknown.
#[test]
fn missed_reports_end_the_lease_and_a_later_report_revives_it() {
    let h = Harness::new(RegistrationConfig::new());
    register_ready(&h, "host-17/agent", "127.0.0.1", 7443);

    h.advance_and_sweep(h.config.lease_ttl + Duration::from_secs(1));
    assert!(matches!(
        h.registry.snapshot()[0].state,
        RegistrationState::Stale { .. }
    ));
    assert_eq!(backend_urls(&h.registry).len(), 0);

    let response = report(
        &h.registry,
        &peer("host-17"),
        "session-1",
        "host-17",
        BackendHealth::BACKEND_HEALTH_READY,
        &["host-17/agent"],
    );
    assert_eq!(
        response.action.as_known(),
        Some(ReportAction::REPORT_ACTION_CONTINUE)
    );
    assert!(matches!(
        h.registry.snapshot()[0].state,
        RegistrationState::Live
    ));

    h.advance_and_sweep(h.config.stale_remove_after + Duration::from_secs(1));
    assert!(
        matches!(
            h.registry.snapshot()[0].state,
            RegistrationState::Stale { .. }
        ),
        "the renewed lease ended again"
    );
    h.advance_and_sweep(h.config.stale_remove_after + Duration::from_secs(1));
    assert!(h.registry.snapshot().is_empty(), "stale entry removed");
    let response = report(
        &h.registry,
        &peer("host-17"),
        "session-1",
        "host-17",
        BackendHealth::BACKEND_HEALTH_READY,
        &["host-17/agent"],
    );
    assert_eq!(
        response.unknown_backend_ids,
        vec!["host-17/agent".to_string()]
    );
}

/// 7.2 Proxy restart: an empty registry answers the first report with
/// `REREGISTER`.
#[test]
fn a_proxy_restart_asks_the_sidecar_to_register_again() {
    let h = Harness::new(RegistrationConfig::new());
    let response = report(
        &h.registry,
        &peer("host-17"),
        "session-1",
        "host-17",
        BackendHealth::BACKEND_HEALTH_READY,
        &["host-17/agent"],
    );
    assert_eq!(
        response.action.as_known(),
        Some(ReportAction::REPORT_ACTION_REREGISTER)
    );
    assert_eq!(
        response.unknown_backend_ids,
        vec!["host-17/agent".to_string()]
    );
}

/// 7.3 Sidecar restart: a new session replaces the old one, keeps the runtime
/// and goes up one generation; the old session is refused.
#[test]
fn a_sidecar_restart_replaces_the_session() {
    let h = Harness::new(RegistrationConfig::new());
    let peer = peer("host-17");
    register(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        vec![spec("host-17/agent", "127.0.0.1", 7443, "1")],
    );
    let before = Arc::clone(&h.registry.snapshot()[0].runtime);
    let response = register(
        &h.registry,
        &peer,
        "session-2",
        "host-17",
        vec![spec("host-17/agent", "127.0.0.1", 7443, "1")],
    );
    assert_eq!(response.results[0].generation, 2);
    assert!(Arc::ptr_eq(&before, &h.registry.snapshot()[0].runtime));
    let error = h
        .registry
        .report(
            &peer,
            &ReportRequest {
                session: Some(SessionRef {
                    session_id: "session-1".to_string(),
                    host_id: "host-17".to_string(),
                    ..Default::default()
                })
                .into(),
                backends: Vec::new(),
                ..Default::default()
            },
        )
        .expect_err("superseded");
    assert!(matches!(
        error,
        foundation_proxy::registration::RegistrationError::SessionSuperseded { .. }
    ));
}

/// 7.4 Sidecar stops for good: the backends go stale and are removed without a
/// `Deregister`.
#[test]
fn a_sidecar_that_stops_loses_its_backends() {
    let h = Harness::new(RegistrationConfig::new());
    register_ready(&h, "host-17/agent", "127.0.0.1", 7443);
    // One sweep ends the lease and marks the entry stale; a later sweep removes
    // it after `stale_remove_after`.
    h.advance_and_sweep(h.config.lease_ttl + Duration::from_secs(1));
    h.advance_and_sweep(h.config.stale_remove_after + Duration::from_secs(1));
    assert_eq!(h.registry.snapshot().len(), 0);
    assert_eq!(backend_urls(&h.registry).len(), 0);
}

/// 7.5 App restart: the sidecar reports `NOT_RUNNING`, then `READY`, and the
/// backend leaves and re-enters rotation.
#[test]
fn an_app_restart_leaves_and_re_enters_rotation() {
    let h = Harness::new(RegistrationConfig::new());
    register_ready(&h, "host-17/agent", "127.0.0.1", 7443);
    assert_eq!(backend_urls(&h.registry).len(), 1);

    report(
        &h.registry,
        &peer("host-17"),
        "session-1",
        "host-17",
        BackendHealth::BACKEND_HEALTH_NOT_RUNNING,
        &["host-17/agent"],
    );
    assert_eq!(backend_urls(&h.registry).len(), 0);

    report(
        &h.registry,
        &peer("host-17"),
        "session-1",
        "host-17",
        BackendHealth::BACKEND_HEALTH_READY,
        &["host-17/agent"],
    );
    assert_eq!(backend_urls(&h.registry).len(), 1);
}

/// 7.6 Redeploy on the same address: the version changes, generation goes up,
/// the runtime is reused.
#[test]
fn a_redeploy_on_the_same_address_keeps_the_runtime() {
    let h = Harness::new(RegistrationConfig::new());
    let peer = peer("host-17");
    register(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        vec![spec("host-17/agent", "127.0.0.1", 7443, "1")],
    );
    let before = Arc::clone(&h.registry.snapshot()[0].runtime);
    let response = register(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        vec![spec("host-17/agent", "127.0.0.1", 7443, "2")],
    );
    assert_eq!(response.results[0].generation, 2);
    assert!(Arc::ptr_eq(&before, &h.registry.snapshot()[0].runtime));
    assert_eq!(h.registry.snapshot()[0].spec.version, "2");
}

/// 7.7 Redeploy on a new address: the new address joins rotation when it is
/// ready, and the old one drains.
#[test]
fn a_redeploy_on_a_new_address_drains_the_old_one() {
    let h = Harness::new(RegistrationConfig::new());
    let peer = peer("host-17");
    register_ready(&h, "host-17/agent", "127.0.0.1", 7443);
    assert_eq!(backend_urls(&h.registry), vec!["http://127.0.0.1:7443"]);

    register(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        vec![spec("host-17/agent", "127.0.0.1", 7444, "2")],
    );
    assert!(
        backend_urls(&h.registry).is_empty(),
        "the new address is not ready yet"
    );
    report(
        &h.registry,
        &peer,
        "session-1",
        "host-17",
        BackendHealth::BACKEND_HEALTH_READY,
        &["host-17/agent"],
    );
    assert_eq!(backend_urls(&h.registry), vec!["http://127.0.0.1:7444"]);

    h.registry.sweep();
    assert_eq!(h.registry.snapshot().len(), 1, "only the new address stays");
}

/// 7.8 Stale or conflicting registration: a live owner refuses, and a stale
/// owner lets the new sidecar take over.
#[test]
fn a_stale_owner_lets_a_new_sidecar_take_over() {
    let h = Harness::new(RegistrationConfig::new());
    let a = peer("host-a");
    let b = peer("host-b");
    register(
        &h.registry,
        &a,
        "session-a",
        "host-a",
        vec![spec("shared/agent", "127.0.0.1", 7443, "1")],
    );

    let refused = register(
        &h.registry,
        &b,
        "session-b",
        "host-b",
        vec![spec("shared/agent", "127.0.0.1", 7444, "1")],
    );
    assert_ne!(refused.results[0].rejected_reason.len(), 0);

    h.advance_and_sweep(h.config.lease_ttl + Duration::from_secs(1));
    let taken = register(
        &h.registry,
        &b,
        "session-b",
        "host-b",
        vec![spec("shared/agent", "127.0.0.1", 7444, "1")],
    );
    assert_eq!(taken.results[0].rejected_reason.len(), 0);
    assert_eq!(h.registry.snapshot()[0].owner_host_id, "host-b");
}

/// 7.9 Address outside the policy: only the bad result is rejected.
#[test]
fn an_address_outside_the_policy_does_not_fail_the_call() {
    let h = Harness::new(RegistrationConfig::new());
    let response = register(
        &h.registry,
        &peer("host-17"),
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
}

/// 7.12 Proxy probe and sidecar disagree: both must pass.
#[test]
fn a_disagreement_between_the_probe_and_the_sidecar_keeps_the_backend_out() {
    let h = Harness::new(RegistrationConfig::new());
    register_ready(&h, "host-17/agent", "127.0.0.1", 7443);
    let runtime = Arc::clone(&h.registry.snapshot()[0].runtime);
    runtime.set_healthy(false);

    let router = h.registry.router();
    let service = router.route(HOST, "/").expect("service");
    assert!(service.pick().is_none());
}

/// 7.13 Too many backends from one sidecar.
#[test]
fn too_many_backends_is_resource_exhausted() {
    let h = Harness::new(RegistrationConfig::new().with_max_backends_per_session(1));
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
    let error = block_on(h.registry.register(&peer("host-17"), &req)).expect_err("limit");
    assert!(matches!(
        error,
        foundation_proxy::registration::RegistrationError::TooManyBackends { .. }
    ));
}
