//! Probes and the health state machine.
//!
//! WHY: the proxy sends traffic only to an app that is ready. The sidecar
//! runs the manifest's startup, liveness and readiness probes with Kubernetes
//! semantics (spec 82 feature 01, section 4.4).
//!
//! WHAT: [`Prober`], [`ProbeVerdict`], [`ProbeOutcome`], the pure
//! per-probe counters, and [`NetworkProber`], which runs HTTP, TCP, gRPC and
//! exec checks over the real network or a command.
//!
//! HOW: the clock, the timers and the probing I/O come from
//! [`Ctx`](foundation_context::Ctx) and the injected [`ProbeExecutor`]. A
//! simulated clock drives every transition in a test.

use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use foundation_connectrpc::{
    ClientExchange, ClientExchangeConfig, CompressionRegistry, GrpcClient, H2Transport,
    ProtocolClient, Spec, Transport,
};
use foundation_context::Ctx;
use foundation_core::url::Uri;
use foundation_core::valtron::sleep_async;
use foundation_netio::shared::http::{
    Proto, RequestDescriptor, SimpleHeader, SimpleHeaders, SimpleMethod, SimpleUrl,
};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};
use futures_util::future::{select, Either};
use futures_util::FutureExt;

use crate::config::RegistrarConfig;
use crate::discovery::{CommandRunner, Discovered, RunHandle};
use crate::manifest::{EntryView, ProbeCheck, ProbeSpec, ProbesBlock};
use crate::proto::v1::BackendHealth;

/// The result of one probe kind after its thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeVerdict {
    /// The probe has not run yet.
    Unknown,
    /// The success threshold is met.
    Passing,
    /// The failure threshold is met.
    Failing,
}

/// The health of one app after its due probes ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutcome {
    /// The health that the sidecar reports.
    pub health: BackendHealth,
    /// The last probe failure text, cut to the configured limit.
    pub detail: String,
}

/// Kubernetes defaults and the values from one probe spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeSettings {
    /// Seconds between checks. Default 10.
    pub period: Duration,
    /// Seconds before one check fails. Default 1.
    pub timeout: Duration,
    /// Failures in a row that make the probe fail. Default 3.
    pub failure_threshold: u32,
    /// Successes in a row that make the probe pass. Default 1.
    pub success_threshold: u32,
    /// Seconds to wait after the app starts. Default 0.
    pub initial_delay: Duration,
}

impl ProbeSettings {
    /// The settings of `spec`, with the Kubernetes defaults for the fields it
    /// leaves out.
    #[must_use]
    pub fn from_spec(spec: &ProbeSpec) -> Self {
        Self {
            period: Duration::from_secs(u64::from(spec.period_secs.unwrap_or(10)).max(1)),
            timeout: Duration::from_secs(u64::from(spec.timeout_secs.unwrap_or(1)).max(1)),
            failure_threshold: spec.failure_threshold.unwrap_or(3).max(1),
            success_threshold: spec.success_threshold.unwrap_or(1).max(1),
            initial_delay: Duration::from_secs(u64::from(spec.initial_delay_secs.unwrap_or(0))),
        }
    }
}

/// Runs one check of one app.
#[async_trait]
pub trait ProbeExecutor: Send + Sync {
    /// Run `check` against `address`. `Ok(())` is a pass; `Err(text)` is a
    /// failure with its detail.
    async fn run(&self, check: &ProbeCheck, address: &str, timeout: Duration)
        -> Result<(), String>;
}

/// Runs probes. Made once at start; the reconcile loop calls it.
pub struct Prober {
    executor: Arc<dyn ProbeExecutor>,
    ctx: Ctx,
    detail_max_bytes: usize,
    apps: HashMap<String, AppState>,
}

impl std::fmt::Debug for Prober {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prober")
            .field("apps", &self.apps.len())
            .finish_non_exhaustive()
    }
}

/// The probe state of one app.
#[derive(Debug, Clone)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each bool is one independent latch of the Kubernetes probe state"
)]
struct AppState {
    handle: Option<RunHandle>,
    was_running: bool,
    startup: Option<ProbeRun>,
    liveness: Option<ProbeRun>,
    readiness: Option<ProbeRun>,
    startup_passed: bool,
    startup_failed: bool,
    liveness_failed: bool,
    readiness_passed: bool,
    health: BackendHealth,
    detail: String,
}

/// The counters and the next due time of one probe kind.
#[derive(Debug, Clone)]
struct ProbeRun {
    settings: ProbeSettings,
    next_due: std::time::Instant,
    successes: u32,
    failures: u32,
    verdict: ProbeVerdict,
}

impl ProbeRun {
    fn new(settings: ProbeSettings, start: std::time::Instant) -> Self {
        Self {
            settings,
            next_due: start + settings.initial_delay,
            successes: 0,
            failures: 0,
            verdict: ProbeVerdict::Unknown,
        }
    }

    /// Record one outcome. `true` when the verdict changed.
    fn record(&mut self, success: bool) -> bool {
        let before = self.verdict;
        if success {
            self.failures = 0;
            self.successes = self.successes.saturating_add(1);
            if self.successes >= self.settings.success_threshold {
                self.verdict = ProbeVerdict::Passing;
            }
        } else {
            self.successes = 0;
            self.failures = self.failures.saturating_add(1);
            if self.failures >= self.settings.failure_threshold {
                self.verdict = ProbeVerdict::Failing;
            }
        }
        before != self.verdict
    }
}

impl Prober {
    /// A prober over `executor`. The `Ctx` supplies the clock and the timer.
    #[must_use]
    pub fn new(ctx: Ctx, executor: Arc<dyn ProbeExecutor>, detail_max_bytes: usize) -> Self {
        Self {
            executor,
            ctx,
            detail_max_bytes: detail_max_bytes.max(1),
            apps: HashMap::new(),
        }
    }

    /// Run the probes that are due for `entry` and return the app's health.
    ///
    /// The result follows the state machine of spec 82 feature 01, section
    /// 4.4: startup gates liveness and readiness, a liveness failure is
    /// `NOT_LIVE`, a readiness failure is `NOT_READY`, and a new process or
    /// container starts again at `STARTING`.
    #[allow(
        clippy::too_many_lines,
        clippy::missing_panics_doc,
        reason = "the four probe rules read as one flow, and the startup spec is present whenever the startup run is"
    )]
    pub async fn observe(
        &mut self,
        entry: &EntryView,
        discovered: &Discovered,
        address: &str,
    ) -> ProbeOutcome {
        let now = self.ctx.clock().now();
        let executor = Arc::clone(&self.executor);
        let detail_max_bytes = self.detail_max_bytes;
        let state = self
            .apps
            .entry(entry.name.clone())
            .or_insert_with(|| AppState::new(entry.probes.as_ref(), None, now));

        let Some(handle) = discovered.handle() else {
            state.was_running = false;
            state.health = BackendHealth::BACKEND_HEALTH_NOT_RUNNING;
            state.detail.clear();
            return state.outcome();
        };

        if !state.was_running || state.handle.as_ref() != Some(handle) {
            *state = AppState::new(entry.probes.as_ref(), Some(handle.clone()), now);
        }
        state.was_running = true;

        let Some(probes) = entry.probes.as_ref() else {
            state.health = BackendHealth::BACKEND_HEALTH_READY;
            state.detail.clear();
            return state.outcome();
        };

        // 1. The startup probe gates liveness and readiness.
        if let Some(startup) = state.startup.as_mut() {
            if !state.startup_passed && !state.startup_failed && now >= startup.next_due {
                let result = executor
                    .run(
                        &probes.startup.as_ref().expect("startup spec").check,
                        address,
                        startup.settings.timeout,
                    )
                    .await;
                startup.next_due = now + startup.settings.period;
                if startup.record(result.is_ok()) {
                    match startup.verdict {
                        ProbeVerdict::Passing => state.startup_passed = true,
                        ProbeVerdict::Failing => state.startup_failed = true,
                        ProbeVerdict::Unknown => {}
                    }
                }
                if let Err(text) = result {
                    state.remember_failure(&text, detail_max_bytes);
                }
            }
            if state.startup_failed {
                state.health = BackendHealth::BACKEND_HEALTH_FAILED_TO_START;
                return state.outcome();
            }
            if !state.startup_passed {
                state.health = BackendHealth::BACKEND_HEALTH_STARTING;
                return state.outcome();
            }
        }

        // 2. Liveness and readiness run on their own periods after startup.
        let mut last_failure = None;
        if let (Some(run), Some(spec)) = (state.liveness.as_mut(), probes.liveness.as_ref()) {
            if now >= run.next_due {
                let result = executor
                    .run(&spec.check, address, run.settings.timeout)
                    .await;
                run.next_due = now + run.settings.period;
                run.record(result.is_ok());
                match run.verdict {
                    ProbeVerdict::Failing => state.liveness_failed = true,
                    ProbeVerdict::Passing => state.liveness_failed = false,
                    ProbeVerdict::Unknown => {}
                }
                if let Err(text) = result {
                    last_failure = Some(text);
                }
            }
        }
        if let (Some(run), Some(spec)) = (state.readiness.as_mut(), probes.readiness.as_ref()) {
            if now >= run.next_due {
                let result = executor
                    .run(&spec.check, address, run.settings.timeout)
                    .await;
                run.next_due = now + run.settings.period;
                run.record(result.is_ok());
                match run.verdict {
                    ProbeVerdict::Failing => state.readiness_passed = false,
                    ProbeVerdict::Passing => state.readiness_passed = true,
                    ProbeVerdict::Unknown => {}
                }
                if let Err(text) = result {
                    last_failure = last_failure.or(Some(text));
                }
            }
        }
        if let Some(text) = last_failure {
            state.remember_failure(&text, detail_max_bytes);
        }

        state.health = if state.liveness_failed {
            BackendHealth::BACKEND_HEALTH_NOT_LIVE
        } else if state.readiness_passed {
            BackendHealth::BACKEND_HEALTH_READY
        } else {
            BackendHealth::BACKEND_HEALTH_NOT_READY
        };
        if state.health == BackendHealth::BACKEND_HEALTH_READY {
            state.detail.clear();
        }
        state.outcome()
    }

    /// Forget one app, for example after it left the manifest.
    pub fn forget(&mut self, entry: &str) {
        self.apps.remove(entry);
    }
}

impl AppState {
    fn new(
        probes: Option<&ProbesBlock>,
        handle: Option<RunHandle>,
        now: std::time::Instant,
    ) -> Self {
        let startup = probes
            .and_then(|probes| probes.startup.as_ref())
            .map(|spec| ProbeRun::new(ProbeSettings::from_spec(spec), now));
        let liveness = probes
            .and_then(|probes| probes.liveness.as_ref())
            .map(|spec| ProbeRun::new(ProbeSettings::from_spec(spec), now));
        let readiness = probes
            .and_then(|probes| probes.readiness.as_ref())
            .map(|spec| ProbeRun::new(ProbeSettings::from_spec(spec), now));
        let health = if handle.is_some() {
            BackendHealth::BACKEND_HEALTH_STARTING
        } else {
            BackendHealth::BACKEND_HEALTH_NOT_RUNNING
        };
        Self {
            handle,
            was_running: true,
            startup,
            liveness,
            readiness,
            startup_passed: probes.is_none_or(|probes| probes.startup.is_none()),
            startup_failed: false,
            liveness_failed: false,
            readiness_passed: probes.is_none_or(|probes| probes.readiness.is_none()),
            health,
            detail: String::new(),
        }
    }

    fn remember_failure(&mut self, text: &str, max_bytes: usize) {
        self.detail = truncate_utf8(text, max_bytes);
    }

    fn outcome(&self) -> ProbeOutcome {
        ProbeOutcome {
            health: self.health,
            detail: self.detail.clone(),
        }
    }
}

/// Cut `text` to at most `max_bytes`, on a character boundary.
fn truncate_utf8(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// A TCP connect, with the host's resolver and socket as the boundary.
#[async_trait]
pub trait TcpConnector: Send + Sync {
    /// Connect to `host:port` within `timeout`.
    ///
    /// # Errors
    /// The operating system's reason, as text.
    async fn connect(&self, host: &str, port: u16, timeout: Duration) -> Result<(), String>;
}

/// The host TCP connector.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemTcpConnector;

#[async_trait]
impl TcpConnector for SystemTcpConnector {
    async fn connect(&self, host: &str, port: u16, timeout: Duration) -> Result<(), String> {
        let authority = format!("{host}:{port}");
        let addresses: Vec<SocketAddr> = authority
            .to_socket_addrs()
            .map_err(|error| format!("resolve {authority}: {error}"))?
            .collect();
        if addresses.is_empty() {
            return Err(format!("resolve {authority}: no address"));
        }
        let mut last = String::new();
        for address in addresses {
            match std::net::TcpStream::connect_timeout(&address, timeout) {
                Ok(_stream) => return Ok(()),
                Err(error) => last = error.to_string(),
            }
        }
        Err(last)
    }
}

/// Runs the four probe kinds over the real network or a command.
pub struct NetworkProber {
    http: DynNetClient,
    tcp: Arc<dyn TcpConnector>,
    commands: Arc<dyn CommandRunner>,
    ctx: Ctx,
    container_mode: bool,
}

impl std::fmt::Debug for NetworkProber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkProber")
            .field("container_mode", &self.container_mode)
            .finish_non_exhaustive()
    }
}

impl NetworkProber {
    /// A prober over a fresh HTTP client, the host TCP connector and `commands`.
    #[must_use]
    pub fn new(ctx: Ctx, commands: Arc<dyn CommandRunner>, container_mode: bool) -> Self {
        let http = HttpClientBuilder::new().build();
        Self {
            http,
            tcp: Arc::new(SystemTcpConnector),
            commands,
            ctx,
            container_mode,
        }
    }

    /// A prober with an injected HTTP client and TCP connector, for a host
    /// that needs different timeouts.
    #[must_use]
    pub fn with_clients(
        ctx: Ctx,
        http: DynNetClient,
        tcp: Arc<dyn TcpConnector>,
        commands: Arc<dyn CommandRunner>,
        container_mode: bool,
    ) -> Self {
        Self {
            http,
            tcp,
            commands,
            ctx,
            container_mode,
        }
    }
}

#[async_trait]
impl ProbeExecutor for NetworkProber {
    async fn run(
        &self,
        check: &ProbeCheck,
        address: &str,
        timeout: Duration,
    ) -> Result<(), String> {
        match check {
            ProbeCheck::Http { port, path } => self.http_probe(address, *port, path, timeout).await,
            ProbeCheck::Tcp { port } => self.tcp.connect(address, *port, timeout).await,
            ProbeCheck::Grpc { port, service } => {
                self.grpc_probe(address, *port, service, timeout).await
            }
            ProbeCheck::Exec { command } => self.exec_probe(command, timeout).await,
        }
    }
}

impl NetworkProber {
    async fn http_probe(
        &self,
        address: &str,
        port: u16,
        path: &str,
        timeout: Duration,
    ) -> Result<(), String> {
        let host = authority(address, port);
        let url = format!("http://{host}{path}");
        let builder = PreparedRequestBuilder::get(&url).map_err(|error| error.to_string())?;
        let request = builder.build();
        let send = self.http.send_async(request).boxed();
        let timer = sleep_async(timeout, &self.ctx).boxed();
        match select(send, timer).await {
            Either::Left((result, _)) => match result {
                Ok(response) => {
                    let code = response.get_status().into_usize();
                    if (200..400).contains(&code) {
                        Ok(())
                    } else {
                        Err(format!("HTTP {code}"))
                    }
                }
                Err(error) => Err(error.to_string()),
            },
            Either::Right(_) => Err(format!("HTTP probe timed out after {timeout:?}")),
        }
    }

    async fn grpc_probe(
        &self,
        address: &str,
        port: u16,
        service: &str,
        timeout: Duration,
    ) -> Result<(), String> {
        let url = format!(
            "http://{}/grpc.health.v1.Health/Check",
            authority(address, port)
        );
        let work = grpc_health_check(url, service.to_string(), timeout, self.ctx.clone()).boxed();
        let timer = sleep_async(timeout, &self.ctx).boxed();
        match select(work, timer).await {
            Either::Left((result, _)) => result,
            Either::Right(_) => Err(format!("gRPC health probe timed out after {timeout:?}")),
        }
    }

    async fn exec_probe(&self, command: &[String], timeout: Duration) -> Result<(), String> {
        if self.container_mode {
            return Err("exec probes are refused in container mode".to_string());
        }
        let Some(program) = command.first() else {
            return Err("exec probe has an empty command".to_string());
        };
        // A process wait blocks its thread. Run it on a dedicated thread, so
        // the probe timer can win and the other probes are not delayed
        // (section 6.13). A process that outlives its check is left to exit on
        // its own; the sidecar never kills an app's tool.
        let commands = Arc::clone(&self.commands);
        let program = program.clone();
        let args = command[1..].to_vec();
        let (sender, receiver) = futures::channel::oneshot::channel();
        std::thread::Builder::new()
            .name("ewe-exec-probe".to_string())
            .spawn(move || {
                let output = futures_lite::future::block_on(commands.run(&program, &args));
                let _ = sender.send(output);
            })
            .map_err(|error| format!("cannot start the exec probe: {error}"))?;
        let run = async move {
            receiver.await.unwrap_or_else(|_| {
                Err(crate::discovery::CommandError {
                    program: "exec probe".to_string(),
                    reason: "the probe thread stopped".to_string(),
                })
            })
        };
        let timer = sleep_async(timeout, &self.ctx).boxed();
        match select(run.boxed(), timer).await {
            Either::Left((result, _)) => match result {
                Ok(output) if output.success() => Ok(()),
                Ok(output) => Err(if output.stderr.is_empty() {
                    format!("exit code {}", output.status)
                } else {
                    output.stderr
                }),
                Err(error) => Err(error.to_string()),
            },
            Either::Right(_) => Err(format!("exec probe timed out after {timeout:?}")),
        }
    }
}

/// `host:port`, with IPv6 brackets.
fn authority(address: &str, port: u16) -> String {
    if address.contains(':') && !address.starts_with('[') {
        format!("[{address}]:{port}")
    } else {
        format!("{address}:{port}")
    }
}

/// `ServingStatus.SERVING` — the value that counts as healthy.
const SERVING: u64 = 0;

/// One `grpc.health.v1.Health/Check` round trip over HTTP/2.
#[allow(
    clippy::too_many_lines,
    reason = "one protocol exchange reads as one flow"
)]
async fn grpc_health_check(
    url: String,
    service: String,
    timeout: Duration,
    ctx: Ctx,
) -> Result<(), String> {
    let uri = Uri::parse(&url).map_err(|error| format!("parse health uri {url}: {error}"))?;
    let mut headers = SimpleHeaders::new();
    headers.insert(
        SimpleHeader::CONTENT_TYPE,
        vec!["application/grpc".to_string()],
    );
    headers.insert(
        SimpleHeader::from("te".to_string()),
        vec!["trailers".to_string()],
    );
    let request = RequestDescriptor {
        proto: Proto::HTTP20,
        request_url: SimpleUrl::url_only(&url),
        request_uri: uri,
        headers,
        method: SimpleMethod::POST,
    };
    let stream = H2Transport::new()
        .with_connect_timeout(timeout)
        .with_handshake_timeout(timeout)
        .with_read_timeout(timeout)
        .open(request)
        .map_err(|error| format!("open h2 health upstream {url}: {error}"))?;
    let compression = CompressionRegistry::new();
    let outbound = GrpcClient
        .new_conn(
            &Spec::empty(true),
            SimpleHeaders::new(),
            stream,
            ClientExchangeConfig {
                cancel: foundation_connectrpc::CancelSignal::new(),
                read_max_bytes: 0,
                compression: &compression,
                send_compressor: None,
                compress_min_bytes: 0,
                send_max_bytes: 0,
            },
        )
        .map_err(|error| format!("build grpc health exchange: {error}"))?;
    let ClientExchange {
        conn,
        reader_task,
        writer_task,
    } = outbound;
    let (mut out_send, mut out_recv) = conn.split();
    let result: Arc<std::sync::Mutex<Option<Result<(), String>>>> =
        Arc::new(std::sync::Mutex::new(None));

    let request_bytes = health_check_request(&service);
    let sender = async move {
        let _ = out_send.send(bytes::Bytes::from(request_bytes)).await;
        let _ = out_send.close_send().await;
    };
    let result_ref = Arc::clone(&result);
    let receiver = async move {
        let outcome = async {
            let mut message: Option<bytes::Bytes> = None;
            loop {
                match out_recv.receive().await {
                    Ok(Some(bytes)) => message = Some(bytes),
                    Ok(None) => break,
                    Err(error) => return Err(format!("health receive failed: {error}")),
                }
            }
            let trailers = out_recv
                .response_trailers()
                .await
                .cloned()
                .unwrap_or_default();
            let grpc_status = trailers
                .get(&SimpleHeader::GRPC_STATUS)
                .and_then(|values| values.first())
                .map(String::as_str);
            if grpc_status != Some("0") {
                return Err(format!("health check grpc-status {grpc_status:?}"));
            }
            if decode_health_status(message.as_deref().unwrap_or(b"")) == SERVING {
                Ok(())
            } else {
                Err("gRPC health status is not SERVING".to_string())
            }
        }
        .await;
        *result_ref.lock().expect("health result mutex") = Some(outcome);
    };

    let work = futures_util::future::join_all(vec![
        sender.boxed(),
        receiver.boxed(),
        reader_task.map(|_| ()).boxed(),
        writer_task.map(|_| ()).boxed(),
    ])
    .boxed();
    let timer = sleep_async(timeout, &ctx).boxed();
    match select(work, timer).await {
        Either::Left(_) => result
            .lock()
            .expect("health result mutex")
            .take()
            .unwrap_or_else(|| Err("gRPC health check produced no result".to_string())),
        Either::Right(_) => Err(format!("gRPC health check timed out after {timeout:?}")),
    }
}

/// Encode `HealthCheckRequest { service }`; an empty service is zero bytes.
fn health_check_request(service: &str) -> Vec<u8> {
    if service.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(service.len() + 11);
    out.push(0x0a);
    push_varint(&mut out, u64::try_from(service.len()).unwrap_or(u64::MAX));
    out.extend_from_slice(service.as_bytes());
    out
}

/// Append one base-128 varint.
fn push_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push(u8::try_from(value & 0x7f).unwrap_or(0) | 0x80);
        value >>= 7;
    }
    out.push(u8::try_from(value).unwrap_or(0));
}

/// Decode `HealthCheckResponse.status` (field 1, varint).
fn decode_health_status(message: &[u8]) -> u64 {
    let mut index = 0;
    while index < message.len() {
        let tag = varint(message, &mut index);
        match tag & 0x07 {
            0 => {
                let value = varint(message, &mut index);
                if tag >> 3 == 1 {
                    return value;
                }
            }
            2 => {
                let length = varint(message, &mut index);
                index = index.saturating_add(usize::try_from(length).unwrap_or(usize::MAX));
            }
            5 => index = index.saturating_add(4),
            1 => index = index.saturating_add(8),
            _ => return SERVING,
        }
    }
    SERVING
}

fn varint(buffer: &[u8], index: &mut usize) -> u64 {
    let mut result = 0u64;
    let mut shift = 0u32;
    while let Some(&byte) = buffer.get(*index) {
        *index += 1;
        result |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 64 {
            break;
        }
    }
    result
}

/// Build the host prober from `config`.
#[must_use]
pub fn host_prober(ctx: Ctx, commands: Arc<dyn CommandRunner>, config: &RegistrarConfig) -> Prober {
    let container_mode = config.mode == crate::config::RegistrarMode::Container;
    Prober::new(
        ctx.clone(),
        Arc::new(NetworkProber::new(ctx, commands, container_mode)),
        config.detail_max_bytes,
    )
}
