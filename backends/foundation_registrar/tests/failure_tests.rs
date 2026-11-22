//! One test for each failure row of spec 82 feature 01, section 6.
//!
//! Rows that the test environment cannot drive (a real network outage, a real
//! revoked certificate) are exercised through the typed error or the real
//! registration authority in `client_tests`.

mod support;

use std::sync::Arc;

use foundation_registrar::{
    AppMetrics, Discovered, MetricsSource, Prober, Reconciler, RegistrarConfig, RegistrarContext,
    RegistrarError, RegistrarMode, RunHandle,
};
use support::{
    entry, probe_spec, probes, proxy_block, write_manifest, FakeCommands, FakeDiscovery,
    FakeExecutor, FakeMetrics, FakeRegistration, Sim,
};

struct Harness {
    sim: Sim,
    _dir: tempfile::TempDir,
    discovery: Arc<FakeDiscovery>,
    executor: Arc<FakeExecutor>,
    client: Arc<FakeRegistration>,
    config: RegistrarConfig,
}

impl Harness {
    fn new(mode: RegistrarMode) -> Self {
        let sim = Sim::new();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let config = RegistrarConfig::new(mode, "host-17")
            .with_proxy("https://127.0.0.1:1")
            .with_state_dir(dir.path())
            .with_manifest_path(dir.path().join("manifest.json"))
            .with_advertise_host("127.0.0.1");
        Self {
            sim,
            _dir: dir,
            discovery: Arc::new(FakeDiscovery::new()),
            executor: Arc::new(FakeExecutor::new()),
            client: Arc::new(FakeRegistration::new()),
            config,
        }
    }

    fn reconciler(&self, metrics: Arc<dyn MetricsSource>) -> Reconciler {
        let context = RegistrarContext::new(
            self.sim.ctx.clone(),
            Arc::new(self.config.clone()),
            Arc::new(FakeCommands::answering("")),
            None,
        );
        let prober = Prober::new(self.sim.ctx.clone(), self.executor.clone(), 256);
        Reconciler::new(
            context,
            self.discovery.clone(),
            prober,
            metrics,
            self.client.clone(),
        )
    }

    fn one_app(&self) {
        write_manifest(
            &self.config.manifest_path,
            &[entry("app", "1", "systemd", Some(proxy_block(7443)), None)],
        );
        self.discovery.set(
            "app",
            Discovered::Running {
                version: "1".to_string(),
                handle: support::process(1),
                started_at_unix_ms: support::SIM_WALL_SECS * 1_000,
                restarts: 0,
                container_ip: None,
            },
        );
    }
}

fn metrics() -> Arc<dyn MetricsSource> {
    Arc::new(FakeMetrics::answering(AppMetrics::default()))
}

#[test]
fn row_6_1_an_unreachable_proxy_is_retried_with_backoff() {
    let harness = Harness::new(RegistrarMode::Host);
    harness.one_app();
    let mut reconciler = harness.reconciler(metrics());
    harness.client.fail_register_times(1);

    let error = futures_lite::future::block_on(reconciler.step()).expect_err("the call fails");
    assert!(matches!(error, RegistrarError::Call { .. }));
    assert_eq!(
        reconciler.backoff().current().as_secs(),
        2,
        "the delay doubles"
    );

    futures_lite::future::block_on(reconciler.step()).expect("the retry succeeds");
    assert_eq!(harness.client.register_calls().len(), 2);
    assert_eq!(reconciler.registered_ids(), vec!["host-17/app".to_string()]);
}

#[test]
fn row_6_2_a_reregister_registers_everything_again() {
    let harness = Harness::new(RegistrarMode::Host);
    harness.one_app();
    harness.client.reregister_once();
    let mut reconciler = harness.reconciler(metrics());
    futures_lite::future::block_on(reconciler.step()).expect("step");
    assert_eq!(harness.client.register_calls().len(), 2);
    assert_eq!(harness.client.report_calls().len(), 2);
}

#[test]
fn row_6_4_a_crash_reports_not_running_and_a_restart_starts_again() {
    let harness = Harness::new(RegistrarMode::Host);
    let manifest = vec![entry(
        "app",
        "1",
        "systemd",
        Some(proxy_block(7443)),
        Some(probes(
            Some(probe_spec(
                foundation_registrar::ProbeCheck::Tcp { port: 7443 },
                1,
                3,
                0,
            )),
            None,
            None,
        )),
    )];
    write_manifest(&harness.config.manifest_path, &manifest);
    harness.discovery.set(
        "app",
        Discovered::Running {
            version: "1".to_string(),
            handle: support::process(1),
            started_at_unix_ms: support::SIM_WALL_SECS * 1_000,
            restarts: 0,
            container_ip: None,
        },
    );
    let mut reconciler = harness.reconciler(metrics());
    harness.executor.push_pass();
    futures_lite::future::block_on(reconciler.step()).expect("running step");

    harness.discovery.set(
        "app",
        Discovered::Stopped {
            version: "1".to_string(),
        },
    );
    futures_lite::future::block_on(reconciler.step()).expect("stopped step");
    let reports = harness.client.report_calls();
    assert_eq!(
        reports[1][0].health.as_known(),
        Some(foundation_registrar::proto::v1::BackendHealth::BACKEND_HEALTH_NOT_RUNNING)
    );

    harness.executor.push_failure("starting");
    harness.discovery.set(
        "app",
        Discovered::Running {
            version: "1".to_string(),
            handle: support::process(2),
            started_at_unix_ms: support::SIM_WALL_SECS * 1_000,
            restarts: 1,
            container_ip: None,
        },
    );
    futures_lite::future::block_on(reconciler.step()).expect("restart step");
    let reports = harness.client.report_calls();
    assert_eq!(
        reports[2][0].health.as_known(),
        Some(foundation_registrar::proto::v1::BackendHealth::BACKEND_HEALTH_STARTING)
    );
}

#[test]
fn row_6_5_an_upgrade_reports_the_new_version() {
    let harness = Harness::new(RegistrarMode::Host);
    harness.one_app();
    let mut reconciler = harness.reconciler(metrics());
    futures_lite::future::block_on(reconciler.step()).expect("first step");

    harness.discovery.set(
        "app",
        Discovered::Running {
            version: "2".to_string(),
            handle: support::process(1),
            started_at_unix_ms: support::SIM_WALL_SECS * 1_000,
            restarts: 0,
            container_ip: None,
        },
    );
    futures_lite::future::block_on(reconciler.step()).expect("upgrade step");
    let registers = harness.client.register_calls();
    assert_eq!(registers.len(), 2);
    assert_eq!(registers[1][0].version, "2");
}

#[test]
fn row_6_6_a_new_address_registers_the_new_address() {
    let harness = Harness::new(RegistrarMode::Container);
    write_manifest(
        &harness.config.manifest_path,
        &[entry("app", "1", "docker", Some(proxy_block(8080)), None)],
    );
    let running_at = |id: &str, ip: &str| Discovered::Running {
        version: "1".to_string(),
        handle: RunHandle::Container { id: id.to_string() },
        started_at_unix_ms: support::SIM_WALL_SECS * 1_000,
        restarts: 0,
        container_ip: Some(ip.parse().expect("ip")),
    };
    harness.discovery.set("app", running_at("c1", "10.0.0.1"));
    let mut reconciler = harness.reconciler(metrics());
    futures_lite::future::block_on(reconciler.step()).expect("first step");

    harness.discovery.set("app", running_at("c2", "10.0.0.2"));
    futures_lite::future::block_on(reconciler.step()).expect("redeploy step");
    let registers = harness.client.register_calls();
    assert_eq!(registers.len(), 2);
    assert_eq!(registers[1][0].address, "10.0.0.2");
}

#[test]
fn row_6_7_an_invalid_manifest_keeps_the_last_good_copy() {
    let harness = Harness::new(RegistrarMode::Host);
    harness.one_app();
    let mut reconciler = harness.reconciler(metrics());
    futures_lite::future::block_on(reconciler.step()).expect("first step");

    std::fs::write(&harness.config.manifest_path, "{ this is not JSON").expect("write bad file");
    assert!(!reconciler.reload(true), "the bad file is refused");
    assert!(reconciler.last_manifest_error().is_some());
    futures_lite::future::block_on(reconciler.step()).expect("the last good manifest still runs");
    assert_eq!(reconciler.registered_ids(), vec!["host-17/app".to_string()]);
    assert_eq!(harness.client.deregister_calls().len(), 0);
}

#[test]
fn row_6_8_a_removed_entry_is_deregistered() {
    let harness = Harness::new(RegistrarMode::Host);
    harness.one_app();
    let mut reconciler = harness.reconciler(metrics());
    futures_lite::future::block_on(reconciler.step()).expect("first step");

    write_manifest(&harness.config.manifest_path, &[]);
    reconciler.reload(true);
    futures_lite::future::block_on(reconciler.step()).expect("second step");
    assert_eq!(
        harness.client.deregister_calls(),
        vec![vec!["host-17/app".to_string()]]
    );
}

#[test]
fn row_6_9_an_owned_backend_is_logged_and_retried() {
    let harness = Harness::new(RegistrarMode::Host);
    harness.one_app();
    harness.client.reject("host-17/app");
    let mut reconciler = harness.reconciler(metrics());
    futures_lite::future::block_on(reconciler.step()).expect("a rejection is not a call error");
    assert_eq!(reconciler.registered_ids().len(), 0);

    harness.client.accept_all();
    futures_lite::future::block_on(reconciler.step()).expect("the retry runs");
    let registers = harness.client.register_calls();
    assert_eq!(registers.len(), 2);
    assert_eq!(reconciler.registered_ids(), vec!["host-17/app".to_string()]);
}

#[test]
fn row_6_9_a_permission_denied_call_is_a_typed_error() {
    let harness = Harness::new(RegistrarMode::Host);
    harness.one_app();
    harness.client.fail_register(RegistrarError::Call {
        code: "PERMISSION_DENIED".to_string(),
        message: "backend belongs to sidecar other-host".to_string(),
    });
    let mut reconciler = harness.reconciler(metrics());
    let error = futures_lite::future::block_on(reconciler.step()).expect_err("denied");
    assert_eq!(
        error,
        RegistrarError::Call {
            code: "PERMISSION_DENIED".to_string(),
            message: "backend belongs to sidecar other-host".to_string(),
        }
    );
}

#[test]
fn row_6_10_a_superseded_session_is_a_typed_error() {
    let harness = Harness::new(RegistrarMode::Host);
    harness.one_app();
    harness
        .client
        .fail_report(RegistrarError::Superseded("host-17".to_string()));
    let mut reconciler = harness.reconciler(metrics());
    let error = futures_lite::future::block_on(reconciler.step()).expect_err("aborted");
    assert_eq!(error, RegistrarError::Superseded("host-17".to_string()));
}

#[test]
fn row_6_12_an_unreadable_metric_is_not_an_error() {
    let harness = Harness::new(RegistrarMode::Host);
    harness.one_app();
    let mut reconciler = harness.reconciler(Arc::new(FakeMetrics::failing()));
    futures_lite::future::block_on(reconciler.step())
        .expect("a metric failure is not a step error");
    let reports = harness.client.report_calls();
    let metrics = reports[0][0].metrics.as_option().expect("metrics field");
    assert_eq!(metrics.cpu_usage_micros, None);
    assert_eq!(metrics.memory_bytes, None);
}
