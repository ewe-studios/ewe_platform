//! Container readiness wait strategies.
//!
//! **WHY:** A container may be "running" but not "ready" — databases need time
//! to initialise, web servers need time to bind ports. The caller needs a
//! declarative way to express readiness without manual polling.
//!
//! **WHAT:** A `WaitFor` enum with four strategies (`Port`, `Http`, `Stdout`,
//! `Composite`) plus `None`. Applied after container start, blocks until the
//! condition is met or the timeout expires.
//!
//! **HOW:** Each variant encapsulates a check and a timeout. Convenience
//! constructors provide sensible defaults.

use std::collections::HashMap;
use std::net::{TcpStream, UdpSocket};
use std::time::{Duration, Instant};

use foundation_core::valtron::sleep_async;
use foundation_deployment_docker::streaming::decoder::{LogFrameDecoder, LogOutput};
use foundation_deployment_docker::DockerClient;
use foundation_netio::http::SimpleHttpClient;
use crate::docker::error::{docker_err, DockerError, DockerResult};

/// A readiness condition for a running container.
#[derive(Debug, Clone)]
pub enum WaitFor {
    /// Wait for a TCP port to accept connections on the host.
    Port { port: u16, timeout: Duration },
    /// Wait for a UDP port to respond to a datagram probe.
    Udp { port: u16, timeout: Duration },
    /// Wait for an HTTP endpoint to return a 2xx status.
    Http { url: String, expected_status: u16, timeout: Duration },
    /// Wait for a message to appear in container stdout/stderr.
    Stdout { message: String, timeout: Duration },
    /// Run multiple strategies in sequence (all must succeed).
    Composite { strategies: Vec<WaitFor> },
    /// No wait.
    None,
}

impl Default for WaitFor {
    fn default() -> Self { Self::None }
}

impl WaitFor {
    pub fn port(port: u16) -> Self {
        Self::Port { port, timeout: Duration::from_secs(30) }
    }
    pub fn port_with_timeout(port: u16, timeout: Duration) -> Self {
        Self::Port { port, timeout }
    }
    pub fn udp(port: u16) -> Self {
        Self::Udp { port, timeout: Duration::from_secs(30) }
    }
    pub fn udp_with_timeout(port: u16, timeout: Duration) -> Self {
        Self::Udp { port, timeout }
    }
    pub fn http(url: impl Into<String>) -> Self {
        Self::Http { url: url.into(), expected_status: 200, timeout: Duration::from_secs(30) }
    }
    pub fn stdout(message: impl Into<String>) -> Self {
        Self::Stdout { message: message.into(), timeout: Duration::from_secs(30) }
    }
    pub fn all(strategies: Vec<WaitFor>) -> Self {
        Self::Composite { strategies }
    }

    /// Returns the timeout duration, or `None` for `WaitFor::None`.
    pub fn timeout(&self) -> Option<Duration> {
        match self {
            Self::Port { timeout, .. }
            | Self::Udp { timeout, .. }
            | Self::Http { timeout, .. }
            | Self::Stdout { timeout, .. } => Some(*timeout),
            Self::Composite { strategies } => {
                strategies.iter().fold(Some(Duration::ZERO), |acc, s| match (acc, s.timeout()) {
                    (Some(a), Some(b)) => Some(a + b),
                    _ => acc,
                })
            }
            Self::None => None,
        }
    }

    /// Apply this wait strategy. Called by ContainerHandle::start_async().
    ///
    /// All waits are async and use valtron's cooperative [`sleep_async`] for
    /// backoff — no thread blocking.
    pub(crate) async fn apply(
        &self,
        client: &DockerClient,
        container_id: &str,
        ports: &HashMap<String, u16>,
    ) -> DockerResult<()> {
        let mut stack = vec![self];
        while let Some(strategy) = stack.pop() {
            match strategy {
                WaitFor::Port { port, timeout } => {
                    wait_for_port(*port, *timeout, ports).await?;
                }
                WaitFor::Udp { port, timeout } => {
                    wait_for_udp(*port, *timeout, ports).await?;
                }
                WaitFor::Stdout { message, timeout } => {
                    wait_for_stdout(client, container_id, message, *timeout).await?;
                }
                WaitFor::Http { url, expected_status, timeout } => {
                    wait_for_http(url, *expected_status, *timeout).await?;
                }
                WaitFor::Composite { strategies } => {
                    for s in strategies.iter().rev() {
                        stack.push(s);
                    }
                }
                WaitFor::None => {}
            }
        }
        Ok(())
    }
}

/// TCP connect loop with exponential backoff. Uses valtron's cooperative
/// [`sleep_async`] — no thread blocking.
async fn wait_for_port(port: u16, timeout: Duration, ports: &HashMap<String, u16>) -> DockerResult<()> {
    let key = format!("{port}/tcp");
    let host_port = ports.get(&key).copied().ok_or_else(|| {
        docker_err(DockerError::InvalidConfig(format!("port {port} was not mapped")))
    })?;

    let start = Instant::now();
    let mut backoff = Duration::from_millis(100);

    loop {
        match TcpStream::connect(format!("127.0.0.1:{host_port}")) {
            Ok(_) => return Ok(()),
            Err(_) if start.elapsed() >= timeout => {
                return Err(docker_err(DockerError::WaitTimeout {
                    strategy: format!("Port({port} via host:{host_port})"),
                    elapsed: start.elapsed(),
                }));
            }
            Err(_) => {
                sleep_async(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(1));
            }
        }
    }
}

/// UDP readiness: send a 1-byte probe to the mapped host port, retry until a
/// response arrives or the timeout elapses. Uses valtron's cooperative
/// [`sleep_async`].
async fn wait_for_udp(port: u16, timeout: Duration, ports: &HashMap<String, u16>) -> DockerResult<()> {
    let key = format!("{port}/udp");
    let host_port = ports.get(&key).copied().ok_or_else(|| {
        docker_err(DockerError::InvalidConfig(format!("UDP port {port} was not mapped")))
    })?;

    let start = Instant::now();
    let mut backoff = Duration::from_millis(100);

    loop {
        match udp_probe(host_port) {
            true => return Ok(()),
            false if start.elapsed() >= timeout => {
                return Err(docker_err(DockerError::WaitTimeout {
                    strategy: format!("Udp({port} via host:{host_port})"),
                    elapsed: start.elapsed(),
                }));
            }
            false => {
                sleep_async(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(1));
            }
        }
    }
}

/// Send a 1-byte probe to `127.0.0.1:host_port` and return true if a response
/// comes back within a short window.  Uses a non-empty payload (0x01) because
/// `socat EXEC:cat` produces no response for an empty datagram — cat reads 0
/// bytes and exits without writing.
fn udp_probe(host_port: u16) -> bool {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return false,
    };
    let _ = socket.set_read_timeout(Some(Duration::from_millis(200)));
    let addr = format!("127.0.0.1:{host_port}");
    if socket.send_to(&[1u8], &addr).is_err() {
        return false;
    }
    let mut buf = [0u8; 16];
    socket.recv_from(&mut buf).is_ok()
}

/// Scan container stdout logs for a message. Polls via
/// [`DockerClient::container_logs`] (follow=false) with
/// [`LogFrameDecoder`] — no real-time streaming needed for a readiness
/// check.
async fn wait_for_stdout(
    client: &DockerClient,
    container_id: &str,
    message: &str,
    timeout: Duration,
) -> DockerResult<()> {
    let start = Instant::now();

    loop {
        let raw = client
            .container_logs(
                container_id,
                false, // follow
                true,  // stdout
                true,  // stderr
                None,  // since
                None,  // until
                false, // timestamps
                None,  // tail (all)
            )
            .await
            .map_err(|e| {
                docker_err(DockerError::Connection(format!("container_logs: {e}")))
            })?;

        let mut decoder = LogFrameDecoder::new();
        decoder.feed(&raw);
        while let Some(frame) = decoder.decode() {
            let text = match frame {
                LogOutput::StdOut { message: msg }
                | LogOutput::StdErr { message: msg } => {
                    String::from_utf8_lossy(&msg).to_string()
                }
                _ => continue,
            };
            if text.contains(message) {
                return Ok(());
            }
        }

        if start.elapsed() >= timeout {
            return Err(docker_err(DockerError::WaitTimeout {
                strategy: format!("Stdout(\"{message}\")"),
                elapsed: start.elapsed(),
            }));
        }

        // Cooperatively sleep between polls.
        sleep_async(Duration::from_millis(500)).await;
    }
}

/// HTTP GET loop via `SimpleHttpClient` — uses the workspace's HTTP stack.
async fn wait_for_http(url: &str, expected_status: u16, timeout: Duration) -> DockerResult<()> {
    let client = SimpleHttpClient::from_system();
    let start = Instant::now();
    let mut backoff = Duration::from_millis(100);

    loop {
        let response = client
            .get(url)
            .map_err(|e| docker_err(DockerError::InvalidConfig(format!("{e}"))))?
            .build_client()
            .map_err(|e| docker_err(DockerError::InvalidConfig(format!("{e}"))))?
            .send_async()
            .await
            .map_err(|e| docker_err(DockerError::InvalidConfig(format!("{e}"))))?;

        if response.get_status().into_usize() == expected_status as usize {
            return Ok(());
        }

        if start.elapsed() >= timeout {
            return Err(docker_err(DockerError::WaitTimeout {
                strategy: format!("Http({url}, expected {expected_status})"),
                elapsed: start.elapsed(),
            }));
        }

        sleep_async(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(1));
    }
}
