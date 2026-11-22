//! Shared test fakes and builders.
//!
//! WHY: the sidecar's side effects are injected, so every unit test drives the
//! real code through a fake clock, a fake runner, a fake Docker API, a fake
//! prober and a fake registration client.
//!
//! WHAT: `Sim`, the fake implementations and the manifest builders.
//!
//! HOW: a fake records its calls in a mutex and returns a scripted value.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use foundation_connectrpc::Code;
use foundation_context::Ctx;
use foundation_core::valtron::SimulatedClock;
use foundation_registrar::client::RegistrationApi;
use foundation_registrar::discovery::docker::ContainerApi;
use foundation_registrar::proto::v1::{
    BackendReport, BackendSpec, DeregisterResponse, RegisterResponse, RegisterResult, ReportAction,
    ReportResponse,
};
use foundation_registrar::{
    AppMetrics, CommandError, CommandOutput, CommandRunner, DeployView, Discovered, Discovery,
    EntryView, ManifestView, MetricsSource, ProbeCheck, ProbeExecutor, ProbeSpec, ProbesBlock,
    ProxyBlock, RegistrarConfig, RegistrarError, RegistrarMode, RouteBlock, RunHandle, SourceView,
};

/// Unix seconds for 2023-11-14, so a simulated certificate is in the past but
/// after the UNIX epoch.
pub const SIM_WALL_SECS: u64 = 1_700_000_000;

/// A simulated clock and its context.
pub struct Sim {
    /// The injected context.
    pub ctx: Ctx,
    /// The clock that the test moves.
    pub clock: Arc<SimulatedClock>,
}

impl Sim {
    /// A context whose clock starts in 2023 and moves only when the test moves
    /// it.
    #[must_use]
    pub fn new() -> Self {
        let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
        clock.advance_wall(Duration::from_secs(SIM_WALL_SECS));
        let ctx = Ctx::fixed(clock.clone());
        foundation_core::env::install(&ctx, |ctx| foundation_core::env::simulated(ctx, 7))
            .expect("simulated Env");
        Self { ctx, clock }
    }

    /// Advance the monotonic clock and the wall clock together.
    pub fn advance(&self, by: Duration) {
        self.clock.advance(by);
        self.clock.advance_wall(by);
    }

    /// Advance only the wall clock, for a certificate lifetime.
    pub fn advance_wall(&self, by: Duration) {
        self.clock.advance_wall(by);
    }
}

impl Default for Sim {
    fn default() -> Self {
        Self::new()
    }
}

/// A real host context with the real environment.
#[must_use]
pub fn host_ctx() -> Ctx {
    let ctx = Ctx::host();
    foundation_core::env::install(&ctx, foundation_core::env::system).expect("host Env");
    ctx
}

/// A scripted `systemctl` runner.
#[derive(Debug, Default)]
pub struct FakeCommands {
    calls: Mutex<Vec<(String, Vec<String>)>>,
    stdout: Mutex<String>,
    status: Mutex<i32>,
}

impl FakeCommands {
    /// A runner that answers with `stdout` and exit code 0.
    #[must_use]
    pub fn answering(stdout: impl Into<String>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            stdout: Mutex::new(stdout.into()),
            status: Mutex::new(0),
        }
    }

    /// A runner that cannot start the program.
    #[must_use]
    pub fn failing() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            stdout: Mutex::new(String::new()),
            status: Mutex::new(-1),
        }
    }

    /// The calls, as `(program, args)`.
    #[must_use]
    pub fn calls(&self) -> Vec<(String, Vec<String>)> {
        self.calls.lock().expect("calls").clone()
    }
}

#[async_trait]
impl CommandRunner for FakeCommands {
    async fn run(&self, program: &str, args: &[String]) -> Result<CommandOutput, CommandError> {
        self.calls
            .lock()
            .expect("calls")
            .push((program.to_string(), args.to_vec()));
        let status = *self.status.lock().expect("status");
        if status < 0 {
            return Err(CommandError {
                program: program.to_string(),
                reason: "the program is absent".to_string(),
            });
        }
        Ok(CommandOutput {
            status,
            stdout: self.stdout.lock().expect("stdout").clone(),
            stderr: String::new(),
        })
    }
}

/// A discovery adapter that returns a scripted view per entry.
#[derive(Default)]
pub struct FakeDiscovery {
    results: Mutex<BTreeMap<String, Discovered>>,
    errors: Mutex<BTreeSet<String>>,
    seen: Mutex<Vec<String>>,
}

impl FakeDiscovery {
    /// An adapter with no entry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the view of `entry`.
    pub fn set(&self, entry: &str, discovered: Discovered) {
        self.results
            .lock()
            .expect("results")
            .insert(entry.to_string(), discovered);
    }

    /// Make discovery of `entry` fail.
    pub fn fail(&self, entry: &str) {
        self.errors
            .lock()
            .expect("errors")
            .insert(entry.to_string());
    }

    /// The entries that discovery was asked about.
    #[must_use]
    pub fn seen(&self) -> Vec<String> {
        self.seen.lock().expect("seen").clone()
    }
}

#[async_trait]
impl Discovery for FakeDiscovery {
    fn kind(&self) -> &'static str {
        "fake"
    }

    async fn discover(
        &self,
        entry: &EntryView,
        _ctx: &foundation_registrar::RegistrarContext,
    ) -> Result<Discovered, RegistrarError> {
        self.seen.lock().expect("seen").push(entry.name.clone());
        if self.errors.lock().expect("errors").contains(&entry.name) {
            return Err(RegistrarError::Discovery {
                entry: entry.name.clone(),
                reason: "the fake refuses".to_string(),
            });
        }
        Ok(self
            .results
            .lock()
            .expect("results")
            .get(&entry.name)
            .cloned()
            .unwrap_or(Discovered::NotInstalled))
    }
}

/// A prober executor with a scripted result queue.
#[derive(Default)]
pub struct FakeExecutor {
    results: Mutex<VecDeque<Result<(), String>>>,
    calls: Mutex<Vec<(String, String, Duration)>>,
}

impl FakeExecutor {
    /// An executor that passes every check.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue one result. An empty queue passes.
    pub fn push(&self, result: Result<(), String>) {
        self.results.lock().expect("results").push_back(result);
    }

    /// Queue a failure with `text`.
    pub fn push_failure(&self, text: impl Into<String>) {
        self.push(Err(text.into()));
    }

    /// Queue a pass.
    pub fn push_pass(&self) {
        self.push(Ok(()));
    }

    /// The calls, as `(kind, address, timeout)`.
    #[must_use]
    pub fn calls(&self) -> Vec<(String, String, Duration)> {
        self.calls.lock().expect("calls").clone()
    }
}

#[async_trait]
impl ProbeExecutor for FakeExecutor {
    async fn run(
        &self,
        check: &ProbeCheck,
        address: &str,
        timeout: Duration,
    ) -> Result<(), String> {
        self.calls.lock().expect("calls").push((
            check.kind().to_string(),
            address.to_string(),
            timeout,
        ));
        self.results
            .lock()
            .expect("results")
            .pop_front()
            .unwrap_or(Ok(()))
    }
}

/// A metrics source with a scripted answer.
pub struct FakeMetrics {
    answer: Mutex<Result<AppMetrics, RegistrarError>>,
    handles: Mutex<Vec<RunHandle>>,
}

impl FakeMetrics {
    /// A source that answers `metrics`.
    #[must_use]
    pub fn answering(metrics: AppMetrics) -> Self {
        Self {
            answer: Mutex::new(Ok(metrics)),
            handles: Mutex::new(Vec::new()),
        }
    }

    /// A source that cannot read.
    #[must_use]
    pub fn failing() -> Self {
        Self {
            answer: Mutex::new(Err(RegistrarError::Metrics("unreadable".to_string()))),
            handles: Mutex::new(Vec::new()),
        }
    }

    /// The handles that were read.
    #[must_use]
    pub fn handles(&self) -> Vec<RunHandle> {
        self.handles.lock().expect("handles").clone()
    }
}

#[async_trait]
impl MetricsSource for FakeMetrics {
    async fn read(&self, handle: &RunHandle) -> Result<AppMetrics, RegistrarError> {
        self.handles.lock().expect("handles").push(handle.clone());
        self.answer.lock().expect("answer").clone()
    }
}

/// One recorded registration call.
#[derive(Debug, Clone)]
pub enum RecordedCall {
    /// `Register` with its specs.
    Register(Vec<BackendSpec>),
    /// `Report` with its reports.
    Report(Vec<BackendReport>),
    /// `Deregister` with its IDs.
    Deregister(Vec<String>),
}

/// A registration client with scripted outcomes and a call log.
#[derive(Default)]
pub struct FakeRegistration {
    calls: Mutex<Vec<RecordedCall>>,
    reject: Mutex<BTreeSet<String>>,
    fail_register: Mutex<Option<RegistrarError>>,
    fail_report: Mutex<Option<RegistrarError>>,
    reregister_once: AtomicBool,
    fail_register_times: AtomicUsize,
    generation: AtomicU64,
    report_interval_ms: AtomicU64,
}

impl FakeRegistration {
    /// A client that accepts everything.
    #[must_use]
    pub fn new() -> Self {
        Self {
            report_interval_ms: AtomicU64::new(1_000),
            ..Self::default()
        }
    }

    /// Reject one backend ID, as the proxy does when another sidecar owns it.
    pub fn reject(&self, backend_id: &str) {
        self.reject
            .lock()
            .expect("reject")
            .insert(backend_id.to_string());
    }

    /// Accept every backend again.
    pub fn accept_all(&self) {
        self.reject.lock().expect("reject").clear();
    }

    /// Fail the next `Register` calls with `error`.
    pub fn fail_register(&self, error: RegistrarError) {
        *self.fail_register.lock().expect("fail_register") = Some(error);
    }

    /// Fail the next `Register` calls `times` times with `UNAVAILABLE`.
    pub fn fail_register_times(&self, times: usize) {
        self.fail_register_times.store(times, Ordering::SeqCst);
    }

    /// Fail the next `Report` with `error`.
    pub fn fail_report(&self, error: RegistrarError) {
        *self.fail_report.lock().expect("fail_report") = Some(error);
    }

    /// Ask for one `REREGISTER` on the next report.
    pub fn reregister_once(&self) {
        self.reregister_once.store(true, Ordering::SeqCst);
    }

    /// Set the report interval that `Register` returns.
    pub fn set_report_interval_ms(&self, millis: u64) {
        self.report_interval_ms.store(millis, Ordering::SeqCst);
    }

    /// The recorded calls.
    #[must_use]
    pub fn calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().expect("calls").clone()
    }

    /// The recorded `Register` calls.
    #[must_use]
    pub fn register_calls(&self) -> Vec<Vec<BackendSpec>> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                RecordedCall::Register(specs) => Some(specs),
                _ => None,
            })
            .collect()
    }

    /// The recorded `Report` calls.
    #[must_use]
    pub fn report_calls(&self) -> Vec<Vec<BackendReport>> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                RecordedCall::Report(reports) => Some(reports),
                _ => None,
            })
            .collect()
    }

    /// The recorded `Deregister` calls.
    #[must_use]
    pub fn deregister_calls(&self) -> Vec<Vec<String>> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                RecordedCall::Deregister(ids) => Some(ids),
                _ => None,
            })
            .collect()
    }
}

#[async_trait]
impl RegistrationApi for FakeRegistration {
    async fn register(&self, specs: &[BackendSpec]) -> Result<RegisterResponse, RegistrarError> {
        self.calls
            .lock()
            .expect("calls")
            .push(RecordedCall::Register(specs.to_vec()));
        if self.fail_register_times.load(Ordering::SeqCst) > 0 {
            self.fail_register_times.fetch_sub(1, Ordering::SeqCst);
            return Err(RegistrarError::Call {
                code: Code::Unavailable.as_str().to_string(),
                message: "the proxy is down".to_string(),
            });
        }
        if let Some(error) = self.fail_register.lock().expect("fail_register").take() {
            return Err(error);
        }
        let reject = self.reject.lock().expect("reject").clone();
        let results = specs
            .iter()
            .map(|spec| {
                if reject.contains(&spec.backend_id) {
                    RegisterResult {
                        backend_id: spec.backend_id.clone(),
                        generation: 0,
                        rejected_reason: "backend belongs to sidecar other-host".to_string(),
                        ..Default::default()
                    }
                } else {
                    RegisterResult {
                        backend_id: spec.backend_id.clone(),
                        generation: self.generation.fetch_add(1, Ordering::SeqCst) + 1,
                        rejected_reason: String::new(),
                        ..Default::default()
                    }
                }
            })
            .collect();
        Ok(RegisterResponse {
            results,
            report_interval_ms: u32::try_from(self.report_interval_ms.load(Ordering::SeqCst))
                .unwrap_or(1_000),
            lease_ttl_ms: 30_000,
            ..Default::default()
        })
    }

    async fn report(&self, reports: &[BackendReport]) -> Result<ReportResponse, RegistrarError> {
        self.calls
            .lock()
            .expect("calls")
            .push(RecordedCall::Report(reports.to_vec()));
        if let Some(error) = self.fail_report.lock().expect("fail_report").take() {
            return Err(error);
        }
        if self.reregister_once.swap(false, Ordering::SeqCst) {
            return Ok(ReportResponse {
                action: ReportAction::REPORT_ACTION_REREGISTER.into(),
                unknown_backend_ids: reports
                    .iter()
                    .map(|report| report.backend_id.clone())
                    .collect(),
                lease_ttl_ms: 30_000,
                ..Default::default()
            });
        }
        Ok(ReportResponse {
            action: ReportAction::REPORT_ACTION_CONTINUE.into(),
            lease_ttl_ms: 30_000,
            ..Default::default()
        })
    }

    async fn deregister(&self, ids: &[String]) -> Result<DeregisterResponse, RegistrarError> {
        self.calls
            .lock()
            .expect("calls")
            .push(RecordedCall::Deregister(ids.to_vec()));
        Ok(DeregisterResponse {
            draining: ids.to_vec(),
            ..Default::default()
        })
    }
}

/// A Docker API fake over the generated response types.
#[derive(Default)]
pub struct FakeContainerApi {
    list: Mutex<serde_json::Value>,
    inspect: Mutex<Option<serde_json::Value>>,
    stats: Mutex<Option<serde_json::Value>>,
    labels: Mutex<Vec<String>>,
    stats_reads: AtomicUsize,
}

impl FakeContainerApi {
    /// A fake with an empty list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: Mutex::new(serde_json::json!([])),
            ..Self::default()
        }
    }

    /// Set the container list.
    pub fn set_list(&self, list: serde_json::Value) {
        *self.list.lock().expect("list") = list;
    }

    /// Set the inspect answer.
    pub fn set_inspect(&self, inspect: serde_json::Value) {
        *self.inspect.lock().expect("inspect") = Some(inspect);
    }

    /// Set the stats answer.
    pub fn set_stats(&self, stats: serde_json::Value) {
        *self.stats.lock().expect("stats") = Some(stats);
    }

    /// The labels that the list was asked for.
    #[must_use]
    pub fn labels(&self) -> Vec<String> {
        self.labels.lock().expect("labels").clone()
    }

    /// How many stats calls ran.
    #[must_use]
    pub fn stats_reads(&self) -> usize {
        self.stats_reads.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ContainerApi for FakeContainerApi {
    async fn container_list_label(
        &self,
        label: &str,
    ) -> Result<Vec<serde_json::Value>, RegistrarError> {
        self.labels.lock().expect("labels").push(label.to_string());
        Ok(self
            .list
            .lock()
            .expect("list")
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    async fn inspect_container(
        &self,
        id: &str,
    ) -> Result<
        foundation_deployment_docker::generated::json::ContainerInspectResponse,
        RegistrarError,
    > {
        let value = self
            .inspect
            .lock()
            .expect("inspect")
            .clone()
            .ok_or_else(|| RegistrarError::Discovery {
                entry: id.to_string(),
                reason: "no inspect answer".to_string(),
            })?;
        serde_json::from_value(value).map_err(|error| RegistrarError::Discovery {
            entry: id.to_string(),
            reason: error.to_string(),
        })
    }

    async fn container_stats(
        &self,
        id: &str,
    ) -> Result<
        foundation_deployment_docker::generated::stats::ContainerStatsResponse,
        RegistrarError,
    > {
        self.stats_reads.fetch_add(1, Ordering::SeqCst);
        let value = self
            .stats
            .lock()
            .expect("stats")
            .clone()
            .ok_or_else(|| RegistrarError::Metrics(format!("no stats for {id}")))?;
        serde_json::from_value(value).map_err(|error| RegistrarError::Metrics(error.to_string()))
    }
}

/// A manifest entry with the given blocks.
#[must_use]
pub fn entry(
    name: &str,
    version: &str,
    deploy_kind: &str,
    proxy: Option<ProxyBlock>,
    probes: Option<ProbesBlock>,
) -> EntryView {
    EntryView {
        name: name.to_string(),
        version: version.to_string(),
        source: SourceView {
            kind: "url".to_string(),
        },
        deploy: DeployView {
            kind: deploy_kind.to_string(),
        },
        proxy,
        probes,
    }
}

/// A proxy block for `agents.example.com` on `port`.
#[must_use]
pub fn proxy_block(port: u16) -> ProxyBlock {
    ProxyBlock {
        service: "agents".to_string(),
        route: RouteBlock {
            host: "agents.example.com".to_string(),
            path_prefix: Some("/".to_string()),
        },
        port,
        protocol: "http".to_string(),
        weight: None,
    }
}

/// A probe spec with the Kubernetes defaults and the named thresholds.
#[must_use]
pub fn probe_spec(
    check: ProbeCheck,
    period_secs: u32,
    failure_threshold: u32,
    initial_delay_secs: u32,
) -> ProbeSpec {
    ProbeSpec {
        check,
        period_secs: Some(period_secs),
        timeout_secs: Some(1),
        failure_threshold: Some(failure_threshold),
        success_threshold: Some(1),
        initial_delay_secs: Some(initial_delay_secs),
    }
}

/// The three probes of one entry.
#[must_use]
pub fn probes(
    startup: Option<ProbeSpec>,
    liveness: Option<ProbeSpec>,
    readiness: Option<ProbeSpec>,
) -> ProbesBlock {
    ProbesBlock {
        startup,
        liveness,
        readiness,
    }
}

/// Write a manifest of `entries` to `path` and return its view.
///
/// # Panics
/// Panics when the file cannot be written.
pub fn write_manifest(path: &std::path::Path, entries: &[EntryView]) -> ManifestView {
    let value = serde_json::json!({
        "manifest_version": 1,
        "entries": entries,
    });
    std::fs::write(
        path,
        serde_json::to_string_pretty(&value).expect("manifest JSON"),
    )
    .expect("write manifest");
    ManifestView {
        manifest_version: 1,
        entries: entries.to_vec(),
    }
}

/// A running process handle.
#[must_use]
pub fn process(pid: u32) -> RunHandle {
    RunHandle::Process {
        pid,
        unit: "ewe-app.service".to_string(),
    }
}

/// A running container handle.
#[must_use]
pub fn container(id: &str) -> RunHandle {
    RunHandle::Container { id: id.to_string() }
}

/// A registrar configuration for a test.
#[must_use]
pub fn test_config(mode: RegistrarMode, host_id: &str, dir: &std::path::Path) -> RegistrarConfig {
    RegistrarConfig::new(mode, host_id)
        .with_proxy("https://127.0.0.1:1")
        .with_state_dir(dir)
        .with_manifest_path(dir.join("manifest.json"))
        .with_advertise_host("127.0.0.1")
}
