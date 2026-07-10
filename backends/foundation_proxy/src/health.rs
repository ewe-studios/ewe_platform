//! Health probes.
//!
//! WHY: Only healthy backends should receive traffic. Each backend with a
//! configured health check is probed on a fixed interval; consecutive
//! successes/failures drive a hysteresis state machine so a single blip neither
//! ejects a good backend nor readmits a flapping one. An unhealthy backend is
//! removed from rotation (`BackendRuntime::set_healthy(false)`) and readmitted
//! only after it passes `healthy_threshold` consecutive probes.
//!
//! WHAT: [`ProbeState`] (the pure consecutive-count state machine, unit-tested
//! without a network) and [`spawn_service_probes`] which runs one probe thread
//! per backend until a shutdown signal fires.
//!
//! HOW: HTTP/HTTPS backends are probed with a real `GET` to the configured
//! path via the shared client; `tcp://` backends are probed with a TCP connect.
//! A 2xx/3xx response (or a successful connect) is a success; anything else is a
//! failure.

use std::net::TcpStream;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use foundation_core::synca::OnSignal;

use crate::config::{BackendProtocol, HealthCheckConfig};
use crate::forward::SharedHttpClient;
use crate::runtime::{BackendRuntime, ServiceRuntime};

/// Consecutive-count health state machine (Decision 14 `HealthCheckConfig`).
///
/// Tracks runs of successes and failures and flips the health verdict only when
/// a run reaches its threshold. `record` returns `Some(new_health)` exactly on
/// the probe that causes a transition, and `None` otherwise.
#[derive(Debug, Clone)]
pub struct ProbeState {
    healthy_threshold: u32,
    unhealthy_threshold: u32,
    consecutive_successes: u32,
    consecutive_failures: u32,
    healthy: bool,
}

impl ProbeState {
    /// Create a state machine. `initial_healthy` seeds the verdict — backends
    /// begin healthy so a service with a slow first probe still serves.
    #[must_use]
    pub fn new(healthy_threshold: u32, unhealthy_threshold: u32, initial_healthy: bool) -> Self {
        Self {
            healthy_threshold: healthy_threshold.max(1),
            unhealthy_threshold: unhealthy_threshold.max(1),
            consecutive_successes: 0,
            consecutive_failures: 0,
            healthy: initial_healthy,
        }
    }

    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.healthy
    }

    /// Record one probe outcome. Returns `Some(new_health)` on a transition.
    ///
    /// A success resets the failure run and, if healthy is not already set,
    /// flips to healthy once `healthy_threshold` consecutive successes land. A
    /// failure is symmetric against `unhealthy_threshold`.
    pub fn record(&mut self, success: bool) -> Option<bool> {
        if success {
            self.consecutive_failures = 0;
            self.consecutive_successes = self.consecutive_successes.saturating_add(1);
            if !self.healthy && self.consecutive_successes >= self.healthy_threshold {
                self.healthy = true;
                return Some(true);
            }
        } else {
            self.consecutive_successes = 0;
            self.consecutive_failures = self.consecutive_failures.saturating_add(1);
            if self.healthy && self.consecutive_failures >= self.unhealthy_threshold {
                self.healthy = false;
                return Some(false);
            }
        }
        None
    }
}

/// Handle to a service's running probe threads; joins them on shutdown.
#[derive(Debug)]
pub struct HealthMonitor {
    handles: Vec<JoinHandle<()>>,
}

impl HealthMonitor {
    /// Join every probe thread. Callers turn the shared shutdown signal on
    /// first, which breaks each probe loop at its next checkpoint.
    pub fn join(self) {
        for handle in self.handles {
            let _ = handle.join();
        }
    }
}

/// Spawn one probe thread per backend of `service` that has a health check.
///
/// Returns a [`HealthMonitor`]; the threads run until `shutdown` is turned on.
/// A service without a `health_check` config spawns nothing — its backends stay
/// at their initial healthy state.
#[must_use]
pub fn spawn_service_probes(
    service: &Arc<ServiceRuntime>,
    client: SharedHttpClient,
    shutdown: Arc<OnSignal>,
) -> HealthMonitor {
    let Some(config) = service.config().health_check.clone() else {
        return HealthMonitor { handles: Vec::new() };
    };

    let mut handles = Vec::new();
    for backend in service.backends() {
        let backend = Arc::clone(backend);
        let config = config.clone();
        let client = client.clone();
        let shutdown = Arc::clone(&shutdown);
        let handle = std::thread::spawn(move || {
            probe_loop(&backend, &config, &client, &shutdown);
        });
        handles.push(handle);
    }
    HealthMonitor { handles }
}

/// Run the probe loop for one backend until shutdown.
fn probe_loop(
    backend: &Arc<BackendRuntime>,
    config: &HealthCheckConfig,
    client: &SharedHttpClient,
    shutdown: &Arc<OnSignal>,
) {
    let mut state = ProbeState::new(
        config.healthy_threshold,
        config.unhealthy_threshold,
        backend.is_healthy(),
    );

    loop {
        if shutdown.probe() {
            return;
        }

        let success = probe_once(backend, config, client);
        if let Some(now_healthy) = state.record(success) {
            backend.set_healthy(now_healthy);
            if now_healthy {
                tracing::info!(url = %backend.url(), "backend became healthy");
            } else {
                tracing::warn!(url = %backend.url(), "backend became unhealthy");
            }
        }

        // Sleep the interval in small slices so shutdown is responsive.
        if !sleep_interruptible(config.interval, shutdown) {
            return;
        }
    }
}

/// Perform a single probe. `true` = success.
fn probe_once(
    backend: &Arc<BackendRuntime>,
    config: &HealthCheckConfig,
    client: &SharedHttpClient,
) -> bool {
    match backend.protocol() {
        BackendProtocol::Tcp => probe_tcp(&backend.target().authority(), config.timeout),
        BackendProtocol::Udp => probe_udp(&backend.target().authority(), config.timeout),
        BackendProtocol::Http | BackendProtocol::Https => {
            probe_http(backend.url(), &config.path, config.timeout, client)
        }
    }
}

/// TCP connect probe with a timeout.
fn probe_tcp(authority: &str, timeout: Duration) -> bool {
    match authority.to_socket_addrs_first() {
        Some(addr) => TcpStream::connect_timeout(&addr, timeout).is_ok(),
        None => TcpStream::connect(authority).is_ok(),
    }
}

/// UDP echo probe: send a 1-byte datagram, return `true` if a response arrives
/// within the timeout.
fn probe_udp(authority: &str, timeout: Duration) -> bool {
    use std::net::UdpSocket;
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else {
        return false;
    };
    let _ = socket.set_read_timeout(Some(timeout));
    if socket.send_to(&[1u8], authority).is_err() {
        return false;
    }
    let mut buf = [0u8; 16];
    socket.recv_from(&mut buf).is_ok()
}

/// HTTP GET probe; success is any 2xx or 3xx response.
fn probe_http(base_url: &str, path: &str, timeout: Duration, client: &SharedHttpClient) -> bool {
    let url = join_url(base_url, path);
    let probe_client = client
        .clone()
        .connect_timeout(timeout)
        .read_timeout(timeout);
    let Ok(builder) = probe_client.get(&url) else {
        return false;
    };
    let Ok(request) = probe_client.request(builder) else {
        return false;
    };
    match request.send() {
        Ok(resp) => {
            let code = resp.get_status().into_usize();
            (200..400).contains(&code)
        }
        Err(err) => {
            tracing::trace!(%url, ?err, "health probe request failed");
            false
        }
    }
}

/// Join a base URL and a health path, avoiding a doubled slash.
fn join_url(base_url: &str, path: &str) -> String {
    let base = base_url.strip_suffix('/').unwrap_or(base_url);
    if path.starts_with('/') {
        format!("{base}{path}")
    } else {
        format!("{base}/{path}")
    }
}

/// Sleep `total` in short slices, returning `false` if shutdown fired.
fn sleep_interruptible(total: Duration, shutdown: &Arc<OnSignal>) -> bool {
    let slice = Duration::from_millis(100);
    let start = Instant::now();
    while start.elapsed() < total {
        if shutdown.probe() {
            return false;
        }
        std::thread::sleep(slice.min(total.saturating_sub(start.elapsed())));
    }
    true
}

/// Minimal helper: resolve the first socket address for `host:port`.
trait FirstSocketAddr {
    fn to_socket_addrs_first(&self) -> Option<std::net::SocketAddr>;
}

impl FirstSocketAddr for str {
    fn to_socket_addrs_first(&self) -> Option<std::net::SocketAddr> {
        use std::net::ToSocketAddrs;
        self.to_socket_addrs().ok().and_then(|mut it| it.next())
    }
}
