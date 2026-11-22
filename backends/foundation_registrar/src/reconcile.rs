//! The reconcile loop: compare the manifest with what runs, and tell the proxy.
//!
//! WHY: one loop per sidecar owns discovery, probing, metrics, registration,
//! reporting and deregistration, so the proxy always has the correct table
//! (spec 82 feature 01, section 4.7).
//!
//! WHAT: [`Observation`], [`ManifestSource`], [`Backoff`] and [`Reconciler`].
//!
//! HOW: each call to [`Reconciler::step`] reads the manifest when it changed,
//! discovers and probes each watched entry, registers new or changed specs,
//! sends one report, acts on `REREGISTER`, and deregisters entries that left
//! the manifest. The binary decides when to call `step` and how long to wait.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::client::RegistrationApi;
use crate::config::{RegistrarConfig, RegistrarMode};
use crate::discovery::{Discovered, Discovery};
use crate::error::RegistrarError;
use crate::manifest::{EntryView, ManifestView, ProxyBlock};
use crate::metrics::{AppMetrics, MetricsSource};
use crate::probe::Prober;
use crate::proto::v1::{BackendMetrics, BackendReport, BackendSpec, ReportAction, Route};
use crate::RegistrarContext;

/// The sidecar's whole view of one app, made each loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// The manifest entry name.
    pub entry: String,
    /// What discovery found.
    pub discovered: Discovered,
    /// The health from the probes.
    pub health: crate::proto::v1::BackendHealth,
    /// The resource use, where the platform provides it.
    pub metrics: AppMetrics,
    /// The last probe failure text, cut to the configured limit.
    pub detail: String,
}

/// The manifest file, with the last good copy kept on a parse error.
#[derive(Debug)]
pub struct ManifestSource {
    path: PathBuf,
    max_bytes: u64,
    last_good: Option<ManifestView>,
    last_mtime: Option<SystemTime>,
    last_error: Option<RegistrarError>,
}

impl ManifestSource {
    /// A source over `path`, reading at most `max_bytes`.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, max_bytes: u64) -> Self {
        Self {
            path: path.into(),
            max_bytes,
            last_good: None,
            last_mtime: None,
            last_error: None,
        }
    }

    /// The last good manifest.
    #[must_use]
    pub fn current(&self) -> Option<&ManifestView> {
        self.last_good.as_ref()
    }

    /// The last read error, kept so a caller can log it each loop.
    #[must_use]
    pub fn last_error(&self) -> Option<&RegistrarError> {
        self.last_error.as_ref()
    }

    /// Read the manifest when the file changed, or always with `force`.
    ///
    /// A parse error keeps the last good manifest, leaves the stored change
    /// time alone, and records the error, so the next call retries.
    /// Returns true when a new manifest was accepted.
    pub fn reload_if_changed(&mut self, force: bool) -> bool {
        let mtime = std::fs::metadata(&self.path)
            .and_then(|metadata| metadata.modified())
            .ok();
        if !force && mtime.is_some() && mtime == self.last_mtime {
            return false;
        }
        match ManifestView::parse_file(&self.path, self.max_bytes) {
            Ok(manifest) => {
                self.last_good = Some(manifest);
                self.last_mtime = mtime;
                self.last_error = None;
                true
            }
            Err(error) => {
                if force || self.last_good.is_none() || mtime != self.last_mtime {
                    tracing::warn!(path = %self.path.display(), %error, "manifest reload failed; keeping the last good copy");
                }
                self.last_error = Some(error);
                false
            }
        }
    }
}

/// Exponential retry delays between `initial` and `max`.
#[derive(Debug, Clone)]
pub struct Backoff {
    initial: Duration,
    max: Duration,
    current: Duration,
}

impl Backoff {
    /// A backoff from `initial` to `max`.
    #[must_use]
    pub fn new(initial: Duration, max: Duration) -> Self {
        Self {
            initial,
            max,
            current: initial,
        }
    }

    /// The delay to wait now, then double it up to `max`.
    pub fn next_delay(&mut self) -> Duration {
        let delay = self.current;
        self.current = self.current.saturating_mul(2).min(self.max);
        delay
    }

    /// The current delay, for a test or a log line.
    #[must_use]
    pub fn current(&self) -> Duration {
        self.current
    }

    /// Start again at `initial`.
    pub fn reset(&mut self) {
        self.current = self.initial;
    }
}

/// The reconcile loop of one sidecar.
pub struct Reconciler {
    context: RegistrarContext,
    discovery: Arc<dyn Discovery>,
    prober: Prober,
    metrics: Arc<dyn MetricsSource>,
    client: Arc<dyn RegistrationApi>,
    manifest: ManifestSource,
    accepted: BTreeMap<String, BackendSpec>,
    registered: BTreeMap<String, BackendSpec>,
    observations: BTreeMap<String, Observation>,
    pending: Vec<BackendSpec>,
    report_interval: Duration,
    backoff: Backoff,
    last_retry_delay: Duration,
}

impl std::fmt::Debug for Reconciler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reconciler")
            .field("registered", &self.registered.len())
            .field("report_interval", &self.report_interval)
            .finish_non_exhaustive()
    }
}

impl Reconciler {
    /// Assemble the loop, reading the manifest once.
    #[must_use]
    pub fn new(
        context: RegistrarContext,
        discovery: Arc<dyn Discovery>,
        prober: Prober,
        metrics: Arc<dyn MetricsSource>,
        client: Arc<dyn RegistrationApi>,
    ) -> Self {
        let config = Arc::clone(&context.config);
        let report_interval = config.default_report_interval;
        let backoff = Backoff::new(config.backoff_initial, config.backoff_max);
        let mut manifest =
            ManifestSource::new(config.manifest_path.clone(), config.max_manifest_bytes);
        manifest.reload_if_changed(true);
        Self {
            context,
            discovery,
            prober,
            metrics,
            client,
            manifest,
            accepted: BTreeMap::new(),
            registered: BTreeMap::new(),
            observations: BTreeMap::new(),
            pending: Vec::new(),
            report_interval,
            backoff,
            last_retry_delay: config.backoff_initial,
        }
    }

    /// The current report interval.
    #[must_use]
    pub fn report_interval(&self) -> Duration {
        self.report_interval
    }

    /// The retry delay that the next failed call waits.
    #[must_use]
    pub fn backoff(&self) -> &Backoff {
        &self.backoff
    }

    /// The delay that the last failed step asked the caller to wait.
    #[must_use]
    pub fn retry_delay(&self) -> Duration {
        self.last_retry_delay
    }

    /// The backend IDs that the proxy accepted.
    #[must_use]
    pub fn registered_ids(&self) -> Vec<String> {
        self.registered.keys().cloned().collect()
    }

    /// The latest observation of one entry.
    #[must_use]
    pub fn observation(&self, entry: &str) -> Option<&Observation> {
        let backend_id = format!("{}/{}", self.context.config.host_id, entry);
        self.observations.get(&backend_id)
    }

    /// The last manifest read error, when the last read failed.
    #[must_use]
    pub fn last_manifest_error(&self) -> Option<&RegistrarError> {
        self.manifest.last_error()
    }

    /// Read the manifest again. A SIGHUP calls this with `force = true`.
    pub fn reload(&mut self, force: bool) -> bool {
        self.manifest.reload_if_changed(force)
    }

    /// Run one iteration of the loop.
    ///
    /// # Errors
    /// The registered call's error. The loop keeps its accepted specs, so the
    /// next call retries them (section 6.1).
    pub async fn step(&mut self) -> Result<(), RegistrarError> {
        match self.step_inner().await {
            Ok(()) => {
                self.backoff.reset();
                Ok(())
            }
            Err(error) => {
                self.last_retry_delay = self.backoff.next_delay();
                tracing::warn!(%error, retry_in = ?self.last_retry_delay, "reconcile step failed");
                Err(error)
            }
        }
    }

    async fn step_inner(&mut self) -> Result<(), RegistrarError> {
        self.manifest.reload_if_changed(false);
        if let Some(manifest) = self.manifest.current().cloned() {
            self.observe_manifest(&manifest).await;
        }
        // A renewal failure must not stop the duty to report; it retries next
        // loop.
        if let Err(error) = self.client.renew_if_due().await {
            tracing::warn!(%error, "client certificate renewal failed");
        }
        self.deregister_removed().await?;
        self.register_dirty().await?;
        let response = self.send_report().await?;
        if response.action.as_known() == Some(ReportAction::REPORT_ACTION_REREGISTER) {
            self.register_all().await?;
            let _ = self.send_report().await?;
        } else if !response.unknown_backend_ids.is_empty() {
            let unknown: Vec<String> = response.unknown_backend_ids.clone();
            let specs: Vec<BackendSpec> = unknown
                .iter()
                .filter_map(|id| self.registered.get(id).cloned())
                .collect();
            if !specs.is_empty() {
                self.register_specs(&specs).await?;
                let _ = self.send_report().await?;
            }
        }
        Ok(())
    }

    /// Discover, probe and measure every watched entry.
    async fn observe_manifest(&mut self, manifest: &ManifestView) {
        let config = Arc::clone(&self.context.config);
        for entry in manifest.watched(&config.entries) {
            let Some(proxy) = entry.proxy.as_ref() else {
                continue;
            };
            if entry.deploy.kind == "none" {
                continue;
            }
            let backend_id = backend_id(&config, entry);
            let discovered = match self.discovery.discover(entry, &self.context).await {
                Ok(discovered) => discovered,
                Err(error) => {
                    tracing::warn!(entry = %entry.name, %error, "discovery failed; keeping the last view");
                    continue;
                }
            };
            let address = match advertise_address(&config, entry, &discovered) {
                Ok(address) => address,
                Err(error) => {
                    tracing::warn!(entry = %entry.name, %error, "no advertise address");
                    continue;
                }
            };
            let outcome = self.prober.observe(entry, &discovered, &address).await;
            let running = discovered.is_running();
            let version = discovered.version().map(str::to_string);
            let metrics = match discovered.handle() {
                Some(handle) if running => self.metrics.read(handle).await.unwrap_or_default(),
                _ => AppMetrics::default(),
            };
            self.observations.insert(
                backend_id.clone(),
                Observation {
                    entry: entry.name.clone(),
                    discovered,
                    health: outcome.health,
                    metrics,
                    detail: outcome.detail,
                },
            );
            if running {
                if let Some(mut spec) = backend_spec(&config, entry, proxy, &address) {
                    if let Some(version) = version {
                        spec.version = version;
                    }
                    let changed = self.accepted.get(&backend_id) != Some(&spec);
                    if changed {
                        self.pending.push(spec);
                    }
                }
            }
        }
    }

    /// Deregister the backends that left the manifest.
    async fn deregister_removed(&mut self) -> Result<(), RegistrarError> {
        // A registered backend is current when the manifest still names it
        // with a proxy block and a deploy kind other than `none`. A discovery
        // failure keeps it in the table, so only a manifest change removes it.
        let manifest_ids: BTreeSet<String> = self.manifest.current().map_or_default(|manifest| {
            manifest
                .entries
                .iter()
                .filter(|entry| {
                    entry.proxy.is_some()
                        && entry.deploy.kind != "none"
                        && self.context.config.watches(&entry.name)
                })
                .map(|entry| backend_id(&self.context.config, entry))
                .collect()
        });
        let removed: Vec<String> = self
            .registered
            .keys()
            .filter(|id| !manifest_ids.contains(*id))
            .cloned()
            .collect();
        if removed.is_empty() {
            return Ok(());
        }
        self.client.deregister(&removed).await?;
        for id in &removed {
            self.registered.remove(id);
            self.accepted.remove(id);
            if let Some(observation) = self.observations.remove(id) {
                self.prober.forget(&observation.entry);
            }
        }
        Ok(())
    }

    /// Send `Register` for every spec that is new or changed.
    async fn register_dirty(&mut self) -> Result<(), RegistrarError> {
        let dirty = std::mem::take(&mut self.pending);
        if dirty.is_empty() {
            return Ok(());
        }
        self.register_specs(&dirty).await
    }

    /// Send `Register` for every registered backend.
    async fn register_all(&mut self) -> Result<(), RegistrarError> {
        let specs: Vec<BackendSpec> = self.registered.values().cloned().collect();
        self.register_specs(&specs).await
    }

    async fn register_specs(&mut self, specs: &[BackendSpec]) -> Result<(), RegistrarError> {
        let response = self.client.register(specs).await?;
        if response.report_interval_ms > 0 {
            self.report_interval = Duration::from_millis(u64::from(response.report_interval_ms));
        }
        for (index, result) in response.results.iter().enumerate() {
            let Some(spec) = specs.get(index) else {
                continue;
            };
            if result.rejected_reason.is_empty() {
                self.accepted.insert(spec.backend_id.clone(), spec.clone());
                self.registered
                    .insert(spec.backend_id.clone(), spec.clone());
            } else {
                tracing::warn!(
                    backend = %spec.backend_id,
                    reason = %result.rejected_reason,
                    "the proxy rejected the backend; retrying next loop"
                );
            }
        }
        Ok(())
    }

    /// Send one report for every registered backend.
    async fn send_report(&mut self) -> Result<crate::proto::v1::ReportResponse, RegistrarError> {
        if self.registered.is_empty() {
            return Ok(crate::proto::v1::ReportResponse::default());
        }
        let now_ms = now_unix_ms(&self.context);
        let mut reports = Vec::with_capacity(self.registered.len());
        for backend_id in self.registered.keys() {
            let Some(observation) = self.observations.get(backend_id) else {
                continue;
            };
            reports.push(backend_report(observation, backend_id, now_ms));
        }
        if reports.is_empty() {
            return Ok(crate::proto::v1::ReportResponse::default());
        }
        self.client.report(&reports).await
    }

    /// Deregister everything at stop, in container mode only.
    ///
    /// In host mode the apps keep running after the sidecar stops, so the
    /// lease ends on its own (section 4.7).
    ///
    /// # Errors
    /// The deregister call's error.
    pub async fn shutdown(&mut self) -> Result<(), RegistrarError> {
        if self.context.config.mode != RegistrarMode::Container {
            return Ok(());
        }
        let ids: Vec<String> = self.registered.keys().cloned().collect();
        if ids.is_empty() {
            return Ok(());
        }
        self.client.deregister(&ids).await?;
        self.registered.clear();
        self.accepted.clear();
        Ok(())
    }
}

/// The backend ID of one entry: `<host_id>/<entry>`.
fn backend_id(config: &RegistrarConfig, entry: &EntryView) -> String {
    format!("{}/{}", config.host_id, entry.name)
}

/// The address that the proxy dials (spec 82 feature 01, section 4.3).
fn advertise_address(
    config: &RegistrarConfig,
    entry: &EntryView,
    discovered: &Discovered,
) -> Result<String, RegistrarError> {
    match config.mode {
        RegistrarMode::Host => config.advertise_host.clone().ok_or_else(|| {
            RegistrarError::Config("advertise_host is required in host mode".to_string())
        }),
        RegistrarMode::Container => {
            if let Discovered::Running {
                container_ip: Some(ip),
                ..
            } = discovered
            {
                return Ok(ip.to_string());
            }
            config
                .pod_ip
                .clone()
                .ok_or_else(|| RegistrarError::Discovery {
                    entry: entry.name.clone(),
                    reason: "no container IP and no pod IP".to_string(),
                })
        }
    }
}

/// A `BackendSpec` for a running entry.
fn backend_spec(
    config: &RegistrarConfig,
    entry: &EntryView,
    proxy: &ProxyBlock,
    address: &str,
) -> Option<BackendSpec> {
    let protocol = proxy.protocol_enum()?;
    Some(BackendSpec {
        backend_id: backend_id(config, entry),
        service: proxy.service.clone(),
        route: buffa::MessageField::some(Route {
            host: proxy.route.host.clone(),
            path_prefix: proxy.path_prefix(),
            ..Default::default()
        }),
        protocol: protocol.into(),
        address: address.to_string(),
        port: u32::from(proxy.port),
        weight: proxy.weight.unwrap_or(1),
        version: String::new(),
        ..Default::default()
    })
}

/// Build one report line.
fn backend_report(observation: &Observation, backend_id: &str, now_ms: u64) -> BackendReport {
    let (restarts, uptime_ms) = match &observation.discovered {
        // A zero start time means that the platform did not give one, so the
        // uptime is left out rather than guessed (section 6.12).
        Discovered::Running {
            started_at_unix_ms,
            restarts,
            ..
        } => (
            Some(*restarts),
            (*started_at_unix_ms > 0).then(|| now_ms.saturating_sub(*started_at_unix_ms)),
        ),
        _ => (None, None),
    };
    BackendReport {
        backend_id: backend_id.to_string(),
        health: observation.health.into(),
        metrics: buffa::MessageField::some(BackendMetrics {
            cpu_usage_micros: observation.metrics.cpu_usage_micros,
            memory_bytes: observation.metrics.memory_bytes,
            memory_limit_bytes: observation.metrics.memory_limit_bytes,
            restarts,
            uptime_ms,
            ..Default::default()
        }),
        detail: observation.detail.clone(),
        ..Default::default()
    }
}

/// The current wall-clock time in Unix milliseconds.
fn now_unix_ms(context: &RegistrarContext) -> u64 {
    u64::try_from(foundation_compact::time::unix_millis(
        context.ctx.clock().wall_now(),
    ))
    .unwrap_or(0)
}
