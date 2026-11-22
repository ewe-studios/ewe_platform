//! The backend registry: sessions, generations, leases and the router swap.
//!
//! WHY: the proxy must learn which backends are live from an authenticated
//! registrar sidecar, not from a fixed config file (spec 82 feature 00). The
//! registry holds every registered backend and every sidecar session, applies
//! the rotation rule, and publishes a new router snapshot in one atomic step.
//!
//! WHAT: [`BackendRegistry`], [`RegisteredBackend`], [`RegistrationState`],
//! [`Session`], [`PeerIdentity`], the owned wire types ([`BackendSpecOwned`],
//! [`BackendMetricsOwned`]) and [`RegistrationError`].
//!
//! HOW: the wire types are the generated ones (`proto.rs`); the registry stores
//! owned copies. Every change happens under one mutex; after a change that can
//! alter rotation the registry builds a fresh immutable [`Router`] and swaps it
//! through a [`RouterHandle`]. A backend whose address did not change keeps its
//! [`BackendRuntime`], so in-flight counts and the proxy health flag survive.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_context::Ctx;
use foundation_netio::shared::client::DnsResolver;

use crate::config::{BackendTarget, ServiceConfig};
use crate::registration::config::RegistrationConfig;
use crate::registration::policy;
use crate::registration::proto::v1::{
    BackendHealth, BackendMetrics, BackendSpec, DeregisterRequest, DeregisterResponse, Protocol,
    RegisterRequest, RegisterResponse, RegisterResult, ReportAction, ReportRequest, ReportResponse,
};
use crate::router::{Router, RouterHandle};
use crate::runtime::{BackendRuntime, ServiceRuntime};

/// What the listener learned about the caller.
///
/// A mutual-TLS listener fills the host from the client certificate common name
/// and records its fingerprint. The local Unix socket records
/// [`Self::local_unix_socket`] and takes the host from the request.
#[derive(Debug, Clone, Default)]
pub struct PeerIdentity {
    /// The `host_id` from the client certificate common name.
    pub host_id: Option<String>,
    /// Lowercase hexadecimal SHA-256 of the client certificate.
    pub fingerprint_sha256: Option<String>,
    /// True when the caller arrived on the local Unix socket.
    pub local_unix_socket: bool,
}

impl PeerIdentity {
    /// A caller that presented an enrolled certificate.
    #[must_use]
    pub fn from_certificate(host_id: impl Into<String>, fingerprint: impl Into<String>) -> Self {
        Self {
            host_id: Some(host_id.into()),
            fingerprint_sha256: Some(fingerprint.into()),
            local_unix_socket: false,
        }
    }

    /// A caller on the local Unix socket.
    #[must_use]
    pub fn local() -> Self {
        Self {
            local_unix_socket: true,
            ..Self::default()
        }
    }

    /// The certificate host name, when the caller presented one.
    #[must_use]
    pub fn host_id(&self) -> Option<&str> {
        self.host_id.as_deref()
    }
}

/// One registered backend spec, owned (the generated message borrows nothing but
/// is rebuilt on every report, so the registry keeps its own copy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendSpecOwned {
    /// `<host_id>/<entry name>`.
    pub backend_id: String,
    /// The proxy service name.
    pub service: String,
    /// Exact host or `*.example.com`.
    pub host: String,
    /// `""` or `"/"` matches everything under the host.
    pub path_prefix: String,
    /// The wire protocol.
    pub protocol: Protocol,
    /// The address that the proxy dials: an IP literal or a DNS name.
    pub address: String,
    /// The port.
    pub port: u16,
    /// Load-balancer weight, at least 1.
    pub weight: u32,
    /// The software version at this address.
    pub version: String,
}

impl BackendSpecOwned {
    /// Copy a generated [`BackendSpec`].
    #[must_use]
    pub fn from_proto(spec: &BackendSpec) -> Self {
        let route = spec.route.as_option();
        Self {
            backend_id: spec.backend_id.clone(),
            service: spec.service.clone(),
            host: route.map_or_else(String::new, |r| r.host.clone()),
            path_prefix: route.map_or_else(String::new, |r| r.path_prefix.clone()),
            protocol: spec
                .protocol
                .as_known()
                .unwrap_or(Protocol::PROTOCOL_UNSPECIFIED),
            address: spec.address.clone(),
            port: u16::try_from(spec.port).unwrap_or(u16::MAX),
            weight: spec.weight.max(1),
            version: spec.version.clone(),
        }
    }

    /// The backend URL that the router dials, for example
    /// `grpc://10.0.4.7:7443`.
    #[must_use]
    pub fn url(&self) -> String {
        let scheme = match self.protocol {
            Protocol::PROTOCOL_HTTPS => "https",
            Protocol::PROTOCOL_CONNECT => "connect",
            Protocol::PROTOCOL_GRPC => "grpc",
            Protocol::PROTOCOL_GRPC_WEB => "grpc-web",
            Protocol::PROTOCOL_HTTP | Protocol::PROTOCOL_UNSPECIFIED => "http",
        };
        format!("{scheme}://{}:{}", self.address, self.port)
    }
}

/// The resource use of one backend, owned. A field that the sidecar could not
/// read is `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackendMetricsOwned {
    /// CPU time used since the process started, in microseconds.
    pub cpu_usage_micros: Option<u64>,
    /// Resident memory now, in bytes.
    pub memory_bytes: Option<u64>,
    /// The memory limit, in bytes.
    pub memory_limit_bytes: Option<u64>,
    /// Restarts seen by the sidecar.
    pub restarts: Option<u32>,
    /// Uptime, in milliseconds.
    pub uptime_ms: Option<u64>,
}

impl BackendMetricsOwned {
    /// Copy a generated [`BackendMetrics`].
    #[must_use]
    pub fn from_proto(metrics: &BackendMetrics) -> Self {
        Self {
            cpu_usage_micros: metrics.cpu_usage_micros,
            memory_bytes: metrics.memory_bytes,
            memory_limit_bytes: metrics.memory_limit_bytes,
            restarts: metrics.restarts,
            uptime_ms: metrics.uptime_ms,
        }
    }
}

/// The registration state of one backend.
///
/// It is separate from the operator state ([`BackendState`]) and from the
/// proxy's own probe result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationState {
    /// Registered and the lease is live. In rotation only when the sidecar
    /// reports `READY`.
    Live,
    /// The lease ended. Out of rotation. Removed after `stale_remove_after`.
    Stale {
        /// When the state changed.
        since: Instant,
    },
    /// Deregistered or replaced. Out of rotation. Removed when no request runs
    /// on it, or after `drain_timeout`.
    Draining {
        /// When the state changed.
        since: Instant,
    },
}

impl RegistrationState {
    /// Whether the backend has a live lease.
    #[must_use]
    pub fn is_live(self) -> bool {
        matches!(self, Self::Live)
    }
}

/// One sidecar run.
#[derive(Debug, Clone)]
pub struct Session {
    /// A stable name of the machine or pod.
    pub host_id: String,
    /// A random ID that the sidecar made at process start.
    pub session_id: String,
    /// When the first `Register` of this run arrived.
    pub started_at: Instant,
    /// The lease ends at this instant unless a report arrives.
    pub lease_expires_at: Instant,
}

/// One registered backend.
#[derive(Debug, Clone)]
pub struct RegisteredBackend {
    /// The last accepted spec.
    pub spec: BackendSpecOwned,
    /// The sidecar that owns it.
    pub owner_host_id: String,
    /// The session that last registered it.
    pub session_id: String,
    /// A number that goes up at each change.
    pub generation: u64,
    /// The health from the last report.
    pub health: BackendHealth,
    /// The metrics from the last report.
    pub metrics: Option<BackendMetricsOwned>,
    /// The registration state.
    pub state: RegistrationState,
    /// When the last report arrived.
    pub last_report_at: Instant,
    /// The runtime state that the router reuses while the address is unchanged.
    pub runtime: Arc<BackendRuntime>,
}

impl RegisteredBackend {
    /// Whether the backend passes the sidecar half of the rotation rule.
    #[must_use]
    pub fn is_in_rotation(&self) -> bool {
        self.state.is_live() && self.health == BackendHealth::BACKEND_HEALTH_READY
    }
}

/// Why a registration call failed.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display)]
pub enum RegistrationError {
    /// The caller is not an enrolled sidecar, or its stated host disagrees with
    /// its certificate.
    #[display("caller is not an enrolled sidecar")]
    Unauthenticated,
    /// The backend belongs to another sidecar.
    #[display("backend {backend_id} belongs to sidecar {owner}")]
    NotOwner {
        /// The backend ID.
        backend_id: String,
        /// The current owner.
        owner: String,
    },
    /// A newer session of the same host replaced this session.
    #[display("session {session_id} was replaced by a newer session of {host_id}")]
    SessionSuperseded {
        /// The host.
        host_id: String,
        /// The replaced session.
        session_id: String,
    },
    /// The request holds more backends than the limit.
    #[display("{count} backends is more than the limit {limit}")]
    TooManyBackends {
        /// The number of backends in the request.
        count: usize,
        /// The configured limit.
        limit: usize,
    },
    /// The request is not valid.
    #[display("invalid request: {_0}")]
    Invalid(String),
}

impl std::error::Error for RegistrationError {}

/// All registered backends and all sidecar sessions.
///
/// Owned by the registration authority; the router never writes to it.
pub struct BackendRegistry {
    inner: Mutex<RegistryState>,
    router: RouterHandle,
    ctx: Ctx,
    config: RegistrationConfig,
    base_services: Vec<Arc<ServiceRuntime>>,
    base_service_names: HashSet<String>,
    base_service_hosts: HashSet<String>,
    resolver: Arc<dyn DnsResolver>,
}

impl std::fmt::Debug for BackendRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackendRegistry")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
struct RegistryState {
    /// The active registration of each backend ID.
    entries: BTreeMap<String, RegisteredBackend>,
    /// Replaced addresses and deregistered backends that are draining or stale.
    retired: Vec<RegisteredBackend>,
    /// Known sessions, live or expired.
    sessions: BTreeMap<String, Session>,
    /// Sessions that a newer session of the same host replaced.
    superseded: BTreeSet<String>,
    /// The current session of each host.
    current_session: BTreeMap<String, String>,
}

impl RegistryState {
    /// Move kept-over old addresses of `backend_id` to `Draining` once the new
    /// address is ready.
    fn retire_replacements(&mut self, backend_id: &str, session_id: &str, now: Instant) {
        for old in &mut self.retired {
            if old.spec.backend_id == backend_id
                && old.session_id == session_id
                && old.state.is_live()
            {
                old.state = RegistrationState::Draining { since: now };
            }
        }
    }

    fn owner_is_live(&self, entry: &RegisteredBackend, now: Instant) -> bool {
        if !entry.state.is_live() || self.superseded.contains(&entry.session_id) {
            return false;
        }
        self.sessions
            .get(&entry.session_id)
            .is_some_and(|session| session.lease_expires_at > now)
    }
}

impl BackendRegistry {
    /// A registry over `base_services` (the config-file services) that publishes
    /// through `router`.
    #[must_use]
    pub fn new(
        base_services: Vec<Arc<ServiceRuntime>>,
        router: RouterHandle,
        ctx: Ctx,
        config: RegistrationConfig,
        resolver: Arc<dyn DnsResolver>,
    ) -> Self {
        let base_service_names = base_services
            .iter()
            .map(|s| s.config().name.clone())
            .collect();
        let base_service_hosts = base_services
            .iter()
            .map(|s| s.config().host.clone())
            .collect();
        Self {
            inner: Mutex::new(RegistryState::default()),
            router,
            ctx,
            config,
            base_services,
            base_service_names,
            base_service_hosts,
            resolver,
        }
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &RegistrationConfig {
        &self.config
    }

    /// The current router.
    #[must_use]
    pub fn router(&self) -> Arc<Router> {
        self.router.load()
    }

    /// The current snapshot of every active registered backend, ordered by
    /// backend ID. For the control socket `list` command.
    ///
    /// # Panics
    /// Panics if the registry mutex is poisoned.
    #[must_use]
    pub fn snapshot(&self) -> Vec<RegisteredBackend> {
        self.inner
            .lock()
            .expect("registry mutex poisoned")
            .entries
            .values()
            .cloned()
            .collect()
    }

    /// Add or update backends. Starts a new session when `session_id` is new.
    ///
    /// # Errors
    /// [`RegistrationError`] for a caller that is not enrolled, a session that a
    /// newer one replaced, or a request over the backend limit. One bad backend
    /// in an otherwise valid call does not fail the call: its
    /// [`RegisterResult`] says why.
    ///
    /// # Panics
    /// Panics if the registry mutex is poisoned.
    pub async fn register(
        &self,
        peer: &PeerIdentity,
        req: &RegisterRequest,
    ) -> Result<RegisterResponse, RegistrationError> {
        let session_ref = req
            .session
            .as_option()
            .ok_or_else(|| RegistrationError::Invalid("session is required".into()))?;
        let host_id = Self::resolve_host(peer, &session_ref.host_id)?;
        let session_id = session_ref.session_id.trim().to_string();
        if session_id.is_empty() {
            return Err(RegistrationError::Invalid("session_id is empty".into()));
        }
        if req.backends.len() > self.config.max_backends_per_session {
            return Err(RegistrationError::TooManyBackends {
                count: req.backends.len(),
                limit: self.config.max_backends_per_session,
            });
        }

        // Policy checks resolve DNS and must not hold the registry lock.
        let mut accepted: Vec<(usize, BackendSpecOwned)> = Vec::new();
        let mut results: Vec<Option<RegisterResult>> = vec![None; req.backends.len()];
        for (index, spec) in req.backends.iter().enumerate() {
            let owned = BackendSpecOwned::from_proto(spec);
            match self.check_spec(&host_id, &owned).await {
                Ok(()) => accepted.push((index, owned)),
                Err(reason) => {
                    results[index] = Some(RegisterResult {
                        backend_id: owned.backend_id,
                        generation: 0,
                        rejected_reason: reason,
                        ..Default::default()
                    });
                }
            }
        }

        let now = self.ctx.clock().now();
        {
            let mut state = self.inner.lock().expect("registry mutex poisoned");
            self.begin_session(&mut state, &host_id, &session_id, now);

            for (index, owned) in accepted {
                let backend_id = owned.backend_id.clone();
                let result =
                    match Self::apply_registration(&mut state, &host_id, &session_id, owned, now) {
                        Ok(generation) => RegisterResult {
                            backend_id,
                            generation,
                            rejected_reason: String::new(),
                            ..Default::default()
                        },
                        Err(reason) => RegisterResult {
                            backend_id,
                            generation: 0,
                            rejected_reason: reason,
                            ..Default::default()
                        },
                    };
                results[index] = Some(result);
            }
        }
        self.publish();

        let results = results
            .into_iter()
            .enumerate()
            .map(|(index, result)| {
                result.unwrap_or_else(|| RegisterResult {
                    backend_id: req.backends[index].backend_id.clone(),
                    generation: 0,
                    rejected_reason: "not processed".to_string(),
                    ..Default::default()
                })
            })
            .collect();

        Ok(RegisterResponse {
            results,
            report_interval_ms: millis_u32(self.config.report_interval),
            lease_ttl_ms: millis_u32(self.config.lease_ttl),
            ..Default::default()
        })
    }

    /// Heartbeat, health and metrics for every backend of this session.
    ///
    /// # Errors
    /// [`RegistrationError::Unauthenticated`] when the certificate host and the
    /// request disagree, [`RegistrationError::SessionSuperseded`] when a newer
    /// session of the same host replaced this one.
    ///
    /// # Panics
    /// Panics if the registry mutex is poisoned.
    pub fn report(
        &self,
        peer: &PeerIdentity,
        req: &ReportRequest,
    ) -> Result<ReportResponse, RegistrationError> {
        let session_ref = req
            .session
            .as_option()
            .ok_or_else(|| RegistrationError::Invalid("session is required".into()))?;
        let host_id = Self::resolve_host(peer, &session_ref.host_id)?;
        let session_id = session_ref.session_id.trim().to_string();
        let now = self.ctx.clock().now();

        let mut state = self.inner.lock().expect("registry mutex poisoned");

        if state.superseded.contains(&session_id) {
            return Err(RegistrationError::SessionSuperseded {
                host_id,
                session_id,
            });
        }
        let Some(session) = state.sessions.get(&session_id) else {
            return Ok(ReportResponse {
                action: ReportAction::REPORT_ACTION_REREGISTER.into(),
                unknown_backend_ids: req
                    .backends
                    .iter()
                    .map(|report| report.backend_id.clone())
                    .collect(),
                lease_ttl_ms: millis_u32(self.config.lease_ttl),
                ..Default::default()
            });
        };
        if session.host_id != host_id {
            return Err(RegistrationError::Unauthenticated);
        }

        let mut unknown = Vec::new();
        for report in &req.backends {
            let health = report
                .health
                .as_known()
                .unwrap_or(BackendHealth::BACKEND_HEALTH_UNSPECIFIED);
            let updated = match state.entries.get_mut(&report.backend_id) {
                Some(entry) if entry.session_id == session_id => {
                    entry.health = health;
                    if let Some(metrics) = report.metrics.as_option() {
                        entry.metrics = Some(BackendMetricsOwned::from_proto(metrics));
                    }
                    entry.last_report_at = now;
                    if matches!(entry.state, RegistrationState::Stale { .. }) {
                        entry.state = RegistrationState::Live;
                    }
                    true
                }
                _ => false,
            };
            if updated {
                // The replacement address is ready, so the old address of the
                // same backend leaves rotation now (7.7: the new address joined
                // first, so no request fails).
                if health == BackendHealth::BACKEND_HEALTH_READY {
                    state.retire_replacements(&report.backend_id, &session_id, now);
                }
            } else {
                unknown.push(report.backend_id.clone());
            }
        }

        // A report from a known session renews its lease.
        if let Some(session) = state.sessions.get_mut(&session_id) {
            session.lease_expires_at = now + self.config.lease_ttl;
        }
        let response = ReportResponse {
            action: ReportAction::REPORT_ACTION_CONTINUE.into(),
            unknown_backend_ids: unknown,
            lease_ttl_ms: millis_u32(self.config.lease_ttl),
            ..Default::default()
        };
        drop(state);
        self.publish();
        Ok(response)
    }

    /// Take backends out of rotation and forget them, after a drain.
    ///
    /// # Errors
    /// [`RegistrationError::Unauthenticated`] when the certificate host and the
    /// request disagree.
    ///
    /// # Panics
    /// Panics if the registry mutex is poisoned.
    pub fn deregister(
        &self,
        peer: &PeerIdentity,
        req: &DeregisterRequest,
    ) -> Result<DeregisterResponse, RegistrationError> {
        let session_ref = req
            .session
            .as_option()
            .ok_or_else(|| RegistrationError::Invalid("session is required".into()))?;
        let host_id = Self::resolve_host(peer, &session_ref.host_id)?;
        let session_id = session_ref.session_id.trim().to_string();
        let now = self.ctx.clock().now();

        let mut state = self.inner.lock().expect("registry mutex poisoned");
        let mut draining = Vec::new();
        for backend_id in &req.backend_ids {
            let owned = state.entries.get(backend_id).is_some_and(|entry| {
                entry.session_id == session_id || entry.owner_host_id == host_id
            });
            if !owned {
                continue;
            }
            if let Some(mut entry) = state.entries.remove(backend_id) {
                entry.state = RegistrationState::Draining { since: now };
                state.retired.push(entry);
                draining.push(backend_id.clone());
            }
        }
        drop(state);
        self.publish();
        Ok(DeregisterResponse {
            draining,
            ..Default::default()
        })
    }

    /// Move every backend of `host_id` to `Draining` (a revoked sidecar).
    /// Returns the number of backends that changed.
    ///
    /// # Panics
    /// Panics if the registry mutex is poisoned.
    pub fn drain_host(&self, host_id: &str) -> usize {
        let now = self.ctx.clock().now();
        let mut state = self.inner.lock().expect("registry mutex poisoned");
        let ids: Vec<String> = state
            .entries
            .iter()
            .filter(|(_, entry)| entry.owner_host_id == host_id)
            .map(|(id, _)| id.clone())
            .collect();
        let count = ids.len();
        for id in ids {
            if let Some(mut entry) = state.entries.remove(&id) {
                entry.state = RegistrationState::Draining { since: now };
                state.retired.push(entry);
            }
        }
        drop(state);
        self.publish();
        count
    }

    /// End expired leases, remove old stale and drained entries, and publish a
    /// new router if anything changed. Called by the sweeper task.
    ///
    /// # Panics
    /// Panics if the registry mutex is poisoned.
    pub fn sweep(&self) {
        let now = self.ctx.clock().now();

        let mut changed = false;
        {
            let mut state = self.inner.lock().expect("registry mutex poisoned");

            // 1. End leases that expired.
            let expired: Vec<String> = state
                .sessions
                .values()
                .filter(|session| session.lease_expires_at <= now)
                .map(|session| session.session_id.clone())
                .collect();
            for session_id in expired {
                for entry in state.entries.values_mut() {
                    if entry.session_id == session_id && entry.state.is_live() {
                        entry.state = RegistrationState::Stale { since: now };
                        changed = true;
                    }
                }
            }

            // 2. Remove stale entries after `stale_remove_after`.
            let before = state.entries.len();
            state.entries.retain(|_, entry| {
                !matches!(entry.state, RegistrationState::Stale { since }
                    if now.saturating_duration_since(since) >= self.config.stale_remove_after)
            });
            changed |= state.entries.len() != before;

            // 3. Remove drained entries after `drain_timeout` or when idle.
            let before = state.retired.len();
            state.retired.retain(|entry| match entry.state {
                RegistrationState::Draining { since } => {
                    entry.runtime.inflight() != 0
                        && now.saturating_duration_since(since) < self.config.drain_timeout
                }
                RegistrationState::Stale { since } => {
                    now.saturating_duration_since(since) < self.config.stale_remove_after
                }
                // A kept-over old address stays until `drain_timeout` even if
                // its replacement never becomes ready.
                RegistrationState::Live => {
                    now.saturating_duration_since(entry.last_report_at) < self.config.drain_timeout
                }
            });
            changed |= state.retired.len() != before;
        }

        if changed {
            self.publish();
        }
    }

    /// Resolve the certificate host and check the request host against it.
    fn resolve_host(peer: &PeerIdentity, requested: &str) -> Result<String, RegistrationError> {
        if let Some(cert_host) = peer.host_id() {
            if !requested.is_empty() && requested != cert_host {
                return Err(RegistrationError::Unauthenticated);
            }
            return Ok(cert_host.to_string());
        }
        if requested.trim().is_empty() {
            return Err(RegistrationError::Invalid("host_id is empty".into()));
        }
        Ok(requested.trim().to_string())
    }

    /// The policy checks of one spec, before the registry lock is taken.
    async fn check_spec(&self, host_id: &str, spec: &BackendSpecOwned) -> Result<(), String> {
        if spec.backend_id.is_empty() {
            return Err("backend_id is empty".into());
        }
        if spec.service.is_empty() {
            return Err("service is empty".into());
        }
        if spec.host.is_empty() {
            return Err("route.host is empty".into());
        }
        policy::check_service(&self.config.policy, host_id, &spec.service)?;
        let is_existing = self.base_service_names.contains(&spec.service)
            || self.base_service_hosts.contains(&spec.host);
        policy::check_route_host(&self.config.policy, &spec.host, is_existing)?;
        policy::check_address(
            &self.config.policy,
            &spec.address,
            spec.port,
            self.resolver.as_ref(),
        )
        .await
    }

    /// Start a session, or replace the old session of the same host.
    fn begin_session(
        &self,
        state: &mut RegistryState,
        host_id: &str,
        session_id: &str,
        now: Instant,
    ) {
        let current = state.current_session.get(host_id).cloned();
        if current.as_deref() == Some(session_id) {
            if let Some(session) = state.sessions.get_mut(session_id) {
                session.lease_expires_at = now + self.config.lease_ttl;
            }
            return;
        }

        if let Some(old) = current {
            state.superseded.insert(old.clone());
            let old_ids: Vec<String> = state
                .entries
                .iter()
                .filter(|(_, entry)| entry.session_id == old)
                .map(|(id, _)| id.clone())
                .collect();
            for id in old_ids {
                if let Some(mut entry) = state.entries.remove(&id) {
                    entry.state = RegistrationState::Draining { since: now };
                    state.retired.push(entry);
                }
            }
            state.sessions.remove(&old);
        }

        state
            .current_session
            .insert(host_id.to_string(), session_id.to_string());
        state.sessions.insert(
            session_id.to_string(),
            Session {
                host_id: host_id.to_string(),
                session_id: session_id.to_string(),
                started_at: now,
                lease_expires_at: now + self.config.lease_ttl,
            },
        );
    }

    /// Apply one accepted registration. Returns the generation, or the reason
    /// that it was refused.
    fn apply_registration(
        state: &mut RegistryState,
        host_id: &str,
        session_id: &str,
        spec: BackendSpecOwned,
        now: Instant,
    ) -> Result<u64, String> {
        let previous = state.entries.get(&spec.backend_id).cloned();
        if let Some(previous) = &previous {
            if previous.owner_host_id != host_id && state.owner_is_live(previous, now) {
                return Err(format!(
                    "backend {} belongs to sidecar {}",
                    spec.backend_id, previous.owner_host_id
                ));
            }
        }

        let url = spec.url();
        let weight = spec.weight.max(1);
        let generation;
        let runtime;
        let mut retire: Option<RegisteredBackend> = None;

        if let Some(previous) = previous {
            if previous.runtime.url() == url.as_str() && previous.runtime.target().weight == weight
            {
                // The address did not change: keep the runtime, so in-flight
                // requests, the proxy health flag and the weight state stay.
                let changed = previous.spec != spec
                    || previous.owner_host_id != host_id
                    || previous.session_id != session_id;
                generation = if changed {
                    previous.generation.saturating_add(1)
                } else {
                    previous.generation
                };
                runtime = Arc::clone(&previous.runtime);
            } else {
                // A new address or weight: the old runtime drains.
                generation = previous.generation.saturating_add(1);
                runtime = Arc::new(BackendRuntime::new(
                    BackendTarget::new(url.clone()).with_weight(weight),
                ));
                retire = Some(previous);
            }
        } else if let Some(retired) = take_retired(state, &spec.backend_id) {
            // A sidecar restart that re-registers the same address: generation
            // goes up, the runtime is reused when the address matches.
            if retired.runtime.url() == url.as_str() && retired.runtime.target().weight == weight {
                runtime = Arc::clone(&retired.runtime);
            } else {
                runtime = Arc::new(BackendRuntime::new(
                    BackendTarget::new(url.clone()).with_weight(weight),
                ));
            }
            generation = retired.generation.saturating_add(1);
        } else {
            generation = 1;
            runtime = Arc::new(BackendRuntime::new(
                BackendTarget::new(url.clone()).with_weight(weight),
            ));
        }

        if let Some(mut old) = retire {
            if old.state.is_live() {
                // Keep the old address in rotation until the new one reports
                // READY (7.7). `last_report_at` is its retirement time, which
                // bounds the keep-over by `drain_timeout`.
                old.last_report_at = now;
            } else {
                old.state = RegistrationState::Draining { since: now };
            }
            state.retired.push(old);
        }

        state.entries.insert(
            spec.backend_id.clone(),
            RegisteredBackend {
                spec,
                owner_host_id: host_id.to_string(),
                session_id: session_id.to_string(),
                generation,
                health: BackendHealth::BACKEND_HEALTH_UNSPECIFIED,
                metrics: None,
                state: RegistrationState::Live,
                last_report_at: now,
                runtime,
            },
        );
        Ok(generation)
    }

    /// Build a router from the config services plus the in-rotation registered
    /// backends and swap it in.
    fn publish(&self) {
        let state = self.inner.lock().expect("registry mutex poisoned");
        let mut groups: BTreeMap<String, Vec<(BackendSpecOwned, Arc<BackendRuntime>)>> =
            BTreeMap::new();
        for entry in state.entries.values().chain(state.retired.iter()) {
            if entry.is_in_rotation() {
                groups
                    .entry(entry.spec.service.clone())
                    .or_default()
                    .push((entry.spec.clone(), Arc::clone(&entry.runtime)));
            }
        }
        drop(state);

        let mut services: Vec<Arc<ServiceRuntime>> =
            Vec::with_capacity(self.base_services.len() + groups.len());
        for base in &self.base_services {
            if let Some(group) = groups.remove(&base.config().name) {
                let mut backends = base.backends().to_vec();
                backends.extend(group.into_iter().map(|(_, runtime)| runtime));
                services.push(Arc::new(ServiceRuntime::with_backends(
                    base.config().clone(),
                    backends,
                )));
            } else {
                services.push(Arc::clone(base));
            }
        }
        for (name, group) in groups {
            let Some((first, _)) = group.first() else {
                continue;
            };
            let mut config = ServiceConfig::new(&name, &first.host);
            if !first.path_prefix.is_empty() && first.path_prefix != "/" {
                config = config.path_prefix(&first.path_prefix);
            }
            let backends = group.into_iter().map(|(_, runtime)| runtime).collect();
            services.push(Arc::new(ServiceRuntime::with_backends(config, backends)));
        }
        self.router.store(Arc::new(Router::new(services)));
    }
}

/// Remove and return the most recent retired entry with `backend_id`.
fn take_retired(state: &mut RegistryState, backend_id: &str) -> Option<RegisteredBackend> {
    let index = state
        .retired
        .iter()
        .rposition(|entry| entry.spec.backend_id == backend_id)?;
    Some(state.retired.remove(index))
}

fn millis_u32(duration: Duration) -> u32 {
    u32::try_from(duration.as_millis()).unwrap_or(u32::MAX)
}
