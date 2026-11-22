//! Reconcile loop tests (spec 82 feature 01, T8).

mod support;

use std::sync::Arc;

use foundation_registrar::{
    AppMetrics, Discovered, MetricsSource, Prober, Reconciler, RegistrarConfig, RegistrarContext,
    RegistrarMode, RunHandle,
};
use support::{
    entry, probe_spec, probes, proxy_block, write_manifest, FakeCommands, FakeDiscovery,
    FakeExecutor, FakeMetrics, FakeRegistration, Sim,
};

struct Fixture {
    sim: Sim,
    dir: tempfile::TempDir,
    discovery: Arc<FakeDiscovery>,
    executor: Arc<FakeExecutor>,
    client: Arc<FakeRegistration>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            sim: Sim::new(),
            dir: tempfile::TempDir::new().expect("temp dir"),
            discovery: Arc::new(FakeDiscovery::new()),
            executor: Arc::new(FakeExecutor::new()),
            client: Arc::new(FakeRegistration::new()),
        }
    }

    /// Build the reconciler over the fakes.
    fn reconciler(&self, config: RegistrarConfig, metrics: Arc<dyn MetricsSource>) -> Reconciler {
        let context = RegistrarContext::new(
            self.sim.ctx.clone(),
            Arc::new(config),
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

    fn config(&self, mode: RegistrarMode) -> RegistrarConfig {
        RegistrarConfig::new(mode, "host-17")
            .with_proxy("https://127.0.0.1:1")
            .with_state_dir(self.dir.path())
            .with_manifest_path(self.dir.path().join("manifest.json"))
            .with_advertise_host("127.0.0.1")
    }
}

fn running(version: &str, handle: RunHandle) -> Discovered {
    Discovered::Running {
        version: version.to_string(),
        handle,
        started_at_unix_ms: support::SIM_WALL_SECS * 1_000 - 5_000,
        restarts: 0,
        container_ip: None,
    }
}

fn running_at(version: &str, handle: RunHandle, ip: &str) -> Discovered {
    Discovered::Running {
        version: version.to_string(),
        handle,
        started_at_unix_ms: support::SIM_WALL_SECS * 1_000 - 5_000,
        restarts: 0,
        container_ip: Some(ip.parse().expect("ip")),
    }
}

#[test]
fn a_new_entry_registers_once_and_reports_each_loop() {
    let fixture = Fixture::new();
    let config = fixture.config(RegistrarMode::Host);
    let manifest = write_manifest(
        &config.manifest_path,
        &[entry("app", "1", "systemd", Some(proxy_block(7443)), None)],
    );
    let _ = manifest;
    fixture
        .discovery
        .set("app", running("1", support::process(1)));
    let mut reconciler = fixture.reconciler(
        config,
        Arc::new(FakeMetrics::answering(AppMetrics::default())),
    );

    futures_lite::future::block_on(reconciler.step()).expect("first step");
    futures_lite::future::block_on(reconciler.step()).expect("second step");

    let registers = fixture.client.register_calls();
    assert_eq!(registers.len(), 1, "only the first loop registers");
    let spec = &registers[0][0];
    assert_eq!(spec.backend_id, "host-17/app");
    assert_eq!(spec.service, "agents");
    assert_eq!(spec.address, "127.0.0.1");
    assert_eq!(spec.port, 7443);
    assert_eq!(spec.version, "1");
    assert_eq!(reconciler.report_interval().as_millis(), 1_000);

    let reports = fixture.client.report_calls();
    assert_eq!(reports.len(), 2, "one report per loop");
    assert_eq!(
        reports[0][0].health.as_known(),
        Some(foundation_registrar::proto::v1::BackendHealth::BACKEND_HEALTH_READY)
    );
}

#[test]
fn the_proxy_report_interval_replaces_the_default() {
    let fixture = Fixture::new();
    let config = fixture
        .config(RegistrarMode::Host)
        .with_default_report_interval(std::time::Duration::from_secs(9));
    write_manifest(
        &config.manifest_path,
        &[entry("app", "1", "systemd", Some(proxy_block(7443)), None)],
    );
    fixture
        .discovery
        .set("app", running("1", support::process(1)));
    fixture.client.set_report_interval_ms(250);
    let mut reconciler = fixture.reconciler(
        config,
        Arc::new(FakeMetrics::answering(AppMetrics::default())),
    );
    assert_eq!(reconciler.report_interval().as_secs(), 9);
    futures_lite::future::block_on(reconciler.step()).expect("step");
    assert_eq!(reconciler.report_interval().as_millis(), 250);
}

#[test]
fn a_new_address_or_version_sends_register_again() {
    let fixture = Fixture::new();
    let config = fixture.config(RegistrarMode::Container);
    write_manifest(
        &config.manifest_path,
        &[entry("app", "1", "docker", Some(proxy_block(8080)), None)],
    );
    fixture
        .discovery
        .set("app", running_at("1", support::container("c1"), "10.0.0.1"));
    let mut reconciler = fixture.reconciler(
        config,
        Arc::new(FakeMetrics::answering(AppMetrics::default())),
    );
    futures_lite::future::block_on(reconciler.step()).expect("first step");

    // A redeploy gives a new container and a new IP.
    fixture
        .discovery
        .set("app", running_at("1", support::container("c2"), "10.0.0.2"));
    futures_lite::future::block_on(reconciler.step()).expect("redeploy step");

    // An upgrade keeps the address and changes the version.
    fixture
        .discovery
        .set("app", running_at("2", support::container("c2"), "10.0.0.2"));
    futures_lite::future::block_on(reconciler.step()).expect("upgrade step");

    let registers = fixture.client.register_calls();
    assert_eq!(registers.len(), 3);
    assert_eq!(registers[0][0].address, "10.0.0.1");
    assert_eq!(registers[1][0].address, "10.0.0.2");
    assert_eq!(registers[2][0].address, "10.0.0.2");
    assert_eq!(registers[2][0].version, "2");
}

#[test]
fn a_removed_entry_is_deregistered() {
    let fixture = Fixture::new();
    let config = fixture.config(RegistrarMode::Host);
    write_manifest(
        &config.manifest_path,
        &[
            entry("app", "1", "systemd", Some(proxy_block(7443)), None),
            entry("other", "1", "systemd", Some(proxy_block(7444)), None),
        ],
    );
    fixture
        .discovery
        .set("app", running("1", support::process(1)));
    fixture
        .discovery
        .set("other", running("1", support::process(2)));
    let mut reconciler = fixture.reconciler(
        config.clone(),
        Arc::new(FakeMetrics::answering(AppMetrics::default())),
    );
    futures_lite::future::block_on(reconciler.step()).expect("first step");
    assert_eq!(reconciler.registered_ids().len(), 2);

    write_manifest(
        &config.manifest_path,
        &[entry("app", "1", "systemd", Some(proxy_block(7443)), None)],
    );
    assert!(reconciler.reload(true), "the new manifest is accepted");
    futures_lite::future::block_on(reconciler.step()).expect("second step");

    let deregisters = fixture.client.deregister_calls();
    assert_eq!(deregisters.len(), 1);
    assert_eq!(deregisters[0], vec!["host-17/other".to_string()]);
    assert_eq!(reconciler.registered_ids(), vec!["host-17/app".to_string()]);
}

#[test]
fn a_stopped_app_is_reported_not_running() {
    let fixture = Fixture::new();
    let config = fixture.config(RegistrarMode::Host);
    write_manifest(
        &config.manifest_path,
        &[entry("app", "1", "systemd", Some(proxy_block(7443)), None)],
    );
    fixture
        .discovery
        .set("app", running("1", support::process(1)));
    let mut reconciler = fixture.reconciler(
        config,
        Arc::new(FakeMetrics::answering(AppMetrics::default())),
    );
    futures_lite::future::block_on(reconciler.step()).expect("running step");

    fixture.discovery.set(
        "app",
        Discovered::Stopped {
            version: "1".to_string(),
        },
    );
    futures_lite::future::block_on(reconciler.step()).expect("stopped step");

    let reports = fixture.client.report_calls();
    assert_eq!(
        reports[1][0].health.as_known(),
        Some(foundation_registrar::proto::v1::BackendHealth::BACKEND_HEALTH_NOT_RUNNING)
    );
    assert_eq!(
        fixture.client.register_calls().len(),
        1,
        "a stopped app sends no new Register"
    );
}

#[test]
fn a_restart_sends_the_app_back_through_starting() {
    let fixture = Fixture::new();
    let config = fixture.config(RegistrarMode::Host);
    write_manifest(
        &config.manifest_path,
        &[entry(
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
        )],
    );
    fixture
        .discovery
        .set("app", running("1", support::process(1)));
    let mut reconciler = fixture.reconciler(
        config,
        Arc::new(FakeMetrics::answering(AppMetrics::default())),
    );
    fixture.executor.push_pass();
    futures_lite::future::block_on(reconciler.step()).expect("first step");

    // The new process starts at STARTING after its first startup failure.
    fixture.executor.push_failure("starting");
    fixture
        .discovery
        .set("app", running("1", support::process(2)));
    futures_lite::future::block_on(reconciler.step()).expect("restart step");
    let reports = fixture.client.report_calls();
    assert_eq!(
        reports[1][0].health.as_known(),
        Some(foundation_registrar::proto::v1::BackendHealth::BACKEND_HEALTH_STARTING)
    );
}

#[test]
fn reregister_registers_every_backend_and_reports_again() {
    let fixture = Fixture::new();
    let config = fixture.config(RegistrarMode::Host);
    write_manifest(
        &config.manifest_path,
        &[entry("app", "1", "systemd", Some(proxy_block(7443)), None)],
    );
    fixture
        .discovery
        .set("app", running("1", support::process(1)));
    fixture.client.reregister_once();
    let mut reconciler = fixture.reconciler(
        config,
        Arc::new(FakeMetrics::answering(AppMetrics::default())),
    );
    futures_lite::future::block_on(reconciler.step()).expect("step");

    assert_eq!(fixture.client.register_calls().len(), 2);
    assert_eq!(fixture.client.report_calls().len(), 2);
}

#[test]
fn an_unreadable_metric_is_left_out_of_the_report() {
    let fixture = Fixture::new();
    let config = fixture.config(RegistrarMode::Host);
    write_manifest(
        &config.manifest_path,
        &[entry("app", "1", "systemd", Some(proxy_block(7443)), None)],
    );
    fixture
        .discovery
        .set("app", running("1", support::process(1)));
    let mut reconciler = fixture.reconciler(config, Arc::new(FakeMetrics::failing()));
    futures_lite::future::block_on(reconciler.step()).expect("step");
    let reports = fixture.client.report_calls();
    let metrics = reports[0][0]
        .metrics
        .as_option()
        .expect("a metrics field is present");
    assert_eq!(metrics.cpu_usage_micros, None);
    assert_eq!(metrics.memory_bytes, None);
    assert_eq!(metrics.memory_limit_bytes, None);
    assert_eq!(metrics.restarts, Some(0));
    assert!(metrics.uptime_ms.is_some());
}

#[test]
fn container_mode_deregisters_at_stop_and_host_mode_sends_nothing() {
    let container_fixture = Fixture::new();
    let config = container_fixture.config(RegistrarMode::Container);
    write_manifest(
        &config.manifest_path,
        &[entry("app", "1", "docker", Some(proxy_block(8080)), None)],
    );
    container_fixture
        .discovery
        .set("app", running_at("1", support::container("c1"), "10.0.0.1"));
    let mut reconciler = container_fixture.reconciler(
        config,
        Arc::new(FakeMetrics::answering(AppMetrics::default())),
    );
    futures_lite::future::block_on(reconciler.step()).expect("step");
    futures_lite::future::block_on(reconciler.shutdown()).expect("shutdown");
    assert_eq!(container_fixture.client.deregister_calls().len(), 1);

    let host_fixture = Fixture::new();
    let config = host_fixture.config(RegistrarMode::Host);
    write_manifest(
        &config.manifest_path,
        &[entry("app", "1", "systemd", Some(proxy_block(7443)), None)],
    );
    host_fixture
        .discovery
        .set("app", running("1", support::process(1)));
    let mut reconciler = host_fixture.reconciler(
        config,
        Arc::new(FakeMetrics::answering(AppMetrics::default())),
    );
    futures_lite::future::block_on(reconciler.step()).expect("step");
    futures_lite::future::block_on(reconciler.shutdown()).expect("shutdown");
    assert_eq!(host_fixture.client.deregister_calls().len(), 0);
}

#[test]
fn only_the_listed_entries_are_watched() {
    let fixture = Fixture::new();
    let mut config = fixture.config(RegistrarMode::Host);
    config.entries = vec!["app".to_string()];
    write_manifest(
        &config.manifest_path,
        &[
            entry("app", "1", "systemd", Some(proxy_block(7443)), None),
            entry("other", "1", "systemd", Some(proxy_block(7444)), None),
        ],
    );
    fixture
        .discovery
        .set("app", running("1", support::process(1)));
    fixture
        .discovery
        .set("other", running("1", support::process(2)));
    let mut reconciler = fixture.reconciler(
        config,
        Arc::new(FakeMetrics::answering(AppMetrics::default())),
    );
    futures_lite::future::block_on(reconciler.step()).expect("step");
    assert_eq!(fixture.discovery.seen(), vec!["app".to_string()]);
    assert_eq!(reconciler.registered_ids(), vec!["host-17/app".to_string()]);
}

#[test]
fn an_entry_without_a_proxy_block_is_watched_but_not_registered() {
    let fixture = Fixture::new();
    let config = fixture.config(RegistrarMode::Host);
    write_manifest(
        &config.manifest_path,
        &[entry("app", "1", "systemd", None, None)],
    );
    fixture
        .discovery
        .set("app", running("1", support::process(1)));
    let mut reconciler = fixture.reconciler(
        config,
        Arc::new(FakeMetrics::answering(AppMetrics::default())),
    );
    futures_lite::future::block_on(reconciler.step()).expect("step");
    assert_eq!(fixture.client.calls().len(), 0);
    assert_eq!(reconciler.registered_ids().len(), 0);
}

#[test]
fn a_changed_manifest_file_is_reloaded_without_a_signal() {
    let fixture = Fixture::new();
    let config = fixture.config(RegistrarMode::Host);
    write_manifest(
        &config.manifest_path,
        &[entry("app", "1", "systemd", Some(proxy_block(7443)), None)],
    );
    fixture
        .discovery
        .set("app", running("1", support::process(1)));
    fixture
        .discovery
        .set("other", running("1", support::process(2)));
    let mut reconciler = fixture.reconciler(
        config.clone(),
        Arc::new(FakeMetrics::answering(AppMetrics::default())),
    );
    futures_lite::future::block_on(reconciler.step()).expect("first step");
    assert_eq!(reconciler.registered_ids(), vec!["host-17/app".to_string()]);

    write_manifest(
        &config.manifest_path,
        &[
            entry("app", "1", "systemd", Some(proxy_block(7443)), None),
            entry("other", "1", "systemd", Some(proxy_block(7444)), None),
        ],
    );
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&config.manifest_path)
        .expect("open manifest");
    file.set_times(
        std::fs::FileTimes::new()
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5)),
    )
    .expect("set mtime");
    futures_lite::future::block_on(reconciler.step()).expect("changed-file step");
    assert_eq!(reconciler.registered_ids().len(), 2);
}
