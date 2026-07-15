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

use foundation_netio::simple_http::client::native::SimpleHttpClient;
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
    /// Composite strategies run each child iteratively (no recursion).
    pub(crate) async fn apply(
        &self,
        docker: &bollard::Docker,
        container_id: &str,
        ports: &HashMap<String, u16>,
    ) -> DockerResult<()> {
        // Flatten composites into a flat list of leaf strategies
        let mut stack = vec![self];
        while let Some(strategy) = stack.pop() {
            match strategy {
                WaitFor::Port { port, timeout } => {
                    wait_for_port(*port, *timeout, ports)?;
                }
                WaitFor::Udp { port, timeout } => {
                    wait_for_udp(*port, *timeout, ports)?;
                }
                WaitFor::Stdout { message, timeout } => {
                    wait_for_stdout(docker, container_id, message, *timeout).await?;
                }
                WaitFor::Http { url, expected_status, timeout } => {
                    wait_for_http(url, *expected_status, *timeout).await?;
                }
                WaitFor::Composite { strategies } => {
                    // Push children in reverse so they execute in original order
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

/// TCP connect loop with exponential backoff.
fn wait_for_port(port: u16, timeout: Duration, ports: &HashMap<String, u16>) -> DockerResult<()> {
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
                std::thread::sleep(backoff);
                backoff = (backoff * 2).min(Duration::from_secs(1));
            }
        }
    }
}

/// UDP readiness: send a 1-byte probe to the mapped host port, retry until a
/// response arrives or the timeout elapses.
fn wait_for_udp(port: u16, timeout: Duration, ports: &HashMap<String, u16>) -> DockerResult<()> {
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
                std::thread::sleep(backoff);
                backoff = (backoff * 2).min(Duration::from_secs(1));
            }
        }
    }
}

/// Send a 1-byte probe to `127.0.0.1:host_port` and return true if a response
/// comes back within a short window.  Uses a non-empty payload (0x01) because
/// `socat EXEC:cat` (and similar UDP echoers) produce no response for an empty
/// datagram — cat reads 0 bytes and exits without writing.
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

/// Scan container stdout logs for a message.
async fn wait_for_stdout(
    docker: &bollard::Docker,
    container_id: &str,
    message: &str,
    timeout: Duration,
) -> DockerResult<()> {
    use bollard::container::LogOutput;
    use bollard::query_parameters::LogsOptionsBuilder;
    use futures_util::StreamExt;
    use tokio::time::timeout as tokio_timeout;

    let options = Some(
        LogsOptionsBuilder::default()
            .follow(true)
            .stdout(true)
            .stderr(true)
            .build(),
    );

    let mut stream = docker.logs(container_id, options);
    let msg = message.to_string();

    let result = tokio_timeout(timeout, async {
        while let Some(Ok(chunk)) = stream.next().await {
            let text = match chunk {
                LogOutput::StdOut { message } => String::from_utf8_lossy(&message).to_string(),
                LogOutput::StdErr { message } => String::from_utf8_lossy(&message).to_string(),
                _ => continue,
            };
            if text.contains(&msg) {
                return Ok(());
            }
        }
        Err(docker_err(DockerError::WaitTimeout {
            strategy: format!("Stdout(\"{msg}\")"),
            elapsed: timeout,
        }))
    })
    .await;

    match result {
        Ok(r) => r,
        Err(_elapsed) => Err(docker_err(DockerError::WaitTimeout {
            strategy: format!("Stdout(\"{msg}\")"),
            elapsed: timeout,
        })),
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

        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(1));
    }
}
