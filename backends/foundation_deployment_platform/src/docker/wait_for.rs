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
use std::net::{Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::time::{Duration, Instant};

use crate::docker::error::{DockerError, DockerResult, classify_docker_client_err, docker_err};
use foundation_core::valtron::sleep_async;
use foundation_deployment_docker::DockerClient;
use foundation_deployment_docker::client::containers_lifecycle::ContainerLogsOptions;
use foundation_deployment_docker::streaming::decoder::LogOutput;
use foundation_netio::{HttpClientBuilder, PreparedRequestBuilder};

/// A readiness condition for a running container.
#[derive(Debug, Clone, Default)]
pub enum WaitFor {
    /// Wait for a TCP port to accept connections on the host.
    Port { port: u16, timeout: Duration },
    /// Wait for a UDP port to respond to a datagram probe.
    Udp { port: u16, timeout: Duration },
    /// Wait for an HTTP endpoint to return a 2xx status.
    Http {
        url: String,
        expected_status: u16,
        timeout: Duration,
    },
    /// Wait for a message to appear in container stdout/stderr.
    Stdout { message: String, timeout: Duration },
    /// Run multiple strategies in sequence (all must succeed).
    Composite { strategies: Vec<WaitFor> },
    /// No wait.
    #[default]
    None,
}

impl WaitFor {
    #[must_use]
    pub fn port(port: u16) -> Self {
        Self::Port {
            port,
            timeout: Duration::from_secs(30),
        }
    }
    #[must_use]
    pub fn port_with_timeout(port: u16, timeout: Duration) -> Self {
        Self::Port { port, timeout }
    }
    #[must_use]
    pub fn udp(port: u16) -> Self {
        Self::Udp {
            port,
            timeout: Duration::from_secs(30),
        }
    }
    #[must_use]
    pub fn udp_with_timeout(port: u16, timeout: Duration) -> Self {
        Self::Udp { port, timeout }
    }
    #[must_use]
    pub fn http(url: impl Into<String>) -> Self {
        Self::Http {
            url: url.into(),
            expected_status: 200,
            timeout: Duration::from_secs(30),
        }
    }
    #[must_use]
    pub fn stdout(message: impl Into<String>) -> Self {
        Self::Stdout {
            message: message.into(),
            timeout: Duration::from_secs(30),
        }
    }
    #[must_use]
    pub fn all(strategies: Vec<WaitFor>) -> Self {
        Self::Composite { strategies }
    }

    /// Returns the timeout duration, or `None` for `WaitFor::None`.
    #[must_use]
    pub fn timeout(&self) -> Option<Duration> {
        match self {
            Self::Port { timeout, .. }
            | Self::Udp { timeout, .. }
            | Self::Http { timeout, .. }
            | Self::Stdout { timeout, .. } => Some(*timeout),
            Self::Composite { strategies } => {
                Some(strategies.iter().filter_map(WaitFor::timeout).sum())
            }
            Self::None => None,
        }
    }

    /// Apply this wait strategy. Called by `ContainerHandle::start_async()`.
    ///
    /// `host` is where the published `ports` are reached from this process
    /// (see [`crate::docker::ContainerHandle::host_ip`]).
    ///
    /// All waits are async and use valtron's cooperative [`sleep_async`] for
    /// backoff — no thread blocking.
    ///
    /// # Errors
    /// Returns an error when the container is not ready before its timeout.
    pub async fn apply(
        &self,
        client: &DockerClient,
        container_id: &str,
        host: Ipv4Addr,
        ports: &HashMap<String, u16>,
    ) -> DockerResult<()> {
        let mut stack = vec![self];
        while let Some(strategy) = stack.pop() {
            match strategy {
                WaitFor::Port { port, timeout } => {
                    wait_for_port_at(host, *port, *timeout, ports).await?;
                }
                WaitFor::Udp { port, timeout } => {
                    wait_for_udp(host, *port, *timeout, ports).await?;
                }
                WaitFor::Stdout { message, timeout } => {
                    wait_for_stdout(client, container_id, message, *timeout).await?;
                }
                WaitFor::Http {
                    url,
                    expected_status,
                    timeout,
                } => {
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

/// Wait until the host port `port` is published and accepts TCP connections on
/// `127.0.0.1`. See [`wait_for_port_at`] for a process that reaches the host
/// elsewhere.
///
/// # Errors
/// Returns a [`DockerError::WaitTimeout`] if the port does not accept a
/// connection before `timeout`, or a [`DockerError::InvalidConfig`] if the
/// port was not mapped.
pub async fn wait_for_port(
    port: u16,
    timeout: Duration,
    ports: &HashMap<String, u16, impl std::hash::BuildHasher>,
) -> DockerResult<()> {
    wait_for_port_at(Ipv4Addr::LOCALHOST, port, timeout, ports).await
}

/// Wait until the host port `port` is published and accepts TCP connections on
/// `host`.
///
/// # Errors
/// Returns a [`DockerError::WaitTimeout`] if the port does not accept a
/// connection before `timeout`, or a [`DockerError::InvalidConfig`] if the
/// port was not mapped.
pub async fn wait_for_port_at(
    host: Ipv4Addr,
    port: u16,
    timeout: Duration,
    ports: &HashMap<String, u16, impl std::hash::BuildHasher>,
) -> DockerResult<()> {
    let key = format!("{port}/tcp");
    let host_port = ports.get(&key).copied().ok_or_else(|| {
        docker_err(DockerError::InvalidConfig(format!(
            "port {port} was not mapped"
        )))
    })?;
    let addr = SocketAddr::from((host, host_port));

    let start = Instant::now();
    let mut backoff = Duration::from_millis(100);

    loop {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(1)) {
            Ok(_) => return Ok(()),
            Err(_) if start.elapsed() >= timeout => {
                return Err(docker_err(DockerError::WaitTimeout {
                    strategy: format!("Port({port} via {addr})"),
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
pub(crate) async fn wait_for_udp(
    host: Ipv4Addr,
    port: u16,
    timeout: Duration,
    ports: &HashMap<String, u16>,
) -> DockerResult<()> {
    let key = format!("{port}/udp");
    let host_port = ports.get(&key).copied().ok_or_else(|| {
        docker_err(DockerError::InvalidConfig(format!(
            "UDP port {port} was not mapped"
        )))
    })?;
    let addr = SocketAddr::from((host, host_port));

    let start = Instant::now();
    let mut backoff = Duration::from_millis(100);

    loop {
        match udp_probe(addr) {
            true => return Ok(()),
            false if start.elapsed() >= timeout => {
                return Err(docker_err(DockerError::WaitTimeout {
                    strategy: format!("Udp({port} via {addr})"),
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

/// Send a 1-byte probe to `addr` and return true if a response comes back
/// within a short window.  Uses a non-empty payload (0x01) because
/// `socat EXEC:cat` produces no response for an empty datagram — cat reads 0
/// bytes and exits without writing.
fn udp_probe(addr: SocketAddr) -> bool {
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else {
        return false;
    };
    // Without the timeout `recv_from` could block the probe forever.
    if socket
        .set_read_timeout(Some(Duration::from_millis(200)))
        .is_err()
    {
        return false;
    }
    if socket.send_to(&[1u8], addr).is_err() {
        return false;
    }
    let mut buf = [0u8; 16];
    socket.recv_from(&mut buf).is_ok()
}

/// What one [`wait_for_stdout`] loop iteration resolved to: the next log frame
/// (or the stream's end), or the remaining timeout firing first.
enum FrameOrTimeout {
    /// The next frame from the log stream; `None` is the stream's end.
    Frame(Option<Result<LogOutput, foundation_deployment_docker::DockerError>>),
    /// No frame arrived before the remaining timeout elapsed.
    TimedOut,
}

/// Scan container stdout logs for a message, following the live log stream.
///
/// One [`DockerClient::container_logs`] call with `follow=true` and
/// `stderr=false` reads the stream once; the loop races each frame against the
/// remaining timeout. The message may span frame boundaries: a rolling buffer
/// of `message.len() + client.limits().max_log_frame` bytes keeps the tail of
/// the stream so a match split across frames is still found. Only
/// [`LogOutput::StdOut`] bytes count; `StdErr`, `StdIn` and `Console` frames
/// are ignored (stderr is not requested, but the stream is checked anyway).
///
/// # Errors
/// Returns a [`DockerError::WaitTimeout`] if the message does not appear before
/// `timeout`, or the daemon's error if the log stream cannot be read.
pub async fn wait_for_stdout(
    client: &DockerClient,
    container_id: &str,
    message: &str,
    timeout: Duration,
) -> DockerResult<()> {
    if message.is_empty() {
        return Ok(());
    }

    let start = Instant::now();
    let options = ContainerLogsOptions {
        follow: true,
        stdout: true,
        stderr: false,
        ..ContainerLogsOptions::default()
    };
    let mut frames = client
        .container_logs(container_id, &options)
        .await
        .map_err(|e| docker_err(classify_docker_client_err("container_logs", e)))?;

    let needle = message.as_bytes();
    // A match that straddles frames needs the last `message.len() - 1` bytes of
    // the previous content plus the new frame. Keeping one whole frame extra
    // (`max_log_frame`) is comfortably more, and byte-safe: the buffer holds
    // raw bytes, never decoded text, so rolling the front can never split a
    // UTF-8 character.
    let cap = needle.len() + client.limits().max_log_frame;
    let mut buf: Vec<u8> = Vec::new();

    loop {
        let remaining = timeout.saturating_sub(start.elapsed());
        // `race` polls both futures in place; whichever resolves first wins and
        // the other is dropped, so a slow stream cannot overrun the timeout.
        let outcome = futures_lite::future::race(
            async { FrameOrTimeout::Frame(frames.next().await) },
            async {
                sleep_async(remaining).await;
                FrameOrTimeout::TimedOut
            },
        )
        .await;

        match outcome {
            // Timed out, or the stream ended (the container stopped) without
            // the message.
            FrameOrTimeout::TimedOut | FrameOrTimeout::Frame(None) => {
                return Err(docker_err(DockerError::WaitTimeout {
                    strategy: format!("Stdout(\"{message}\")"),
                    elapsed: start.elapsed(),
                }));
            }
            FrameOrTimeout::Frame(Some(frame)) => {
                let frame = frame
                    .map_err(|e| docker_err(classify_docker_client_err("container_logs", e)))?;
                if let LogOutput::StdOut { message: msg } = frame {
                    buf.extend_from_slice(&msg);
                    if buf.len() > cap {
                        let excess = buf.len() - cap;
                        buf.drain(..excess);
                    }
                    if buf.windows(needle.len()).any(|w| w == needle) {
                        return Ok(());
                    }
                }
            }
        }
    }
}

/// HTTP GET loop over the platform network client (`HttpClientBuilder`).
/// Only the status is checked; the body is never read.
async fn wait_for_http(url: &str, expected_status: u16, timeout: Duration) -> DockerResult<()> {
    let client = HttpClientBuilder::new().build();
    let start = Instant::now();
    let mut backoff = Duration::from_millis(100);

    loop {
        let request = PreparedRequestBuilder::get(url)
            .map_err(|e| docker_err(DockerError::InvalidConfig(format!("{e}"))))?
            .build();
        let response = client
            .send_async(request)
            .await
            .map_err(|e| docker_err(DockerError::InvalidConfig(format!("{e}"))))?;
        let status: usize = response.get_status().into();

        if status == usize::from(expected_status) {
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
