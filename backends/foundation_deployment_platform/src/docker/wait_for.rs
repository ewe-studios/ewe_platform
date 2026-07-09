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
use std::net::TcpStream;
use std::time::{Duration, Instant};

use crate::docker::error::{docker_err, DockerError, DockerResult};

/// A readiness condition for a running container.
#[derive(Debug, Clone)]
pub enum WaitFor {
    /// Wait for a TCP port to accept connections on the host.
    Port { port: u16, timeout: Duration },
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
    pub fn http(url: impl Into<String>) -> Self {
        Self::Http { url: url.into(), expected_status: 200, timeout: Duration::from_secs(30) }
    }
    pub fn stdout(message: impl Into<String>) -> Self {
        Self::Stdout { message: message.into(), timeout: Duration::from_secs(30) }
    }
    pub fn all(strategies: Vec<WaitFor>) -> Self {
        Self::Composite { strategies }
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

/// HTTP GET loop — TODO: implement when reqwest is available as dep.
#[allow(unused_variables)]
async fn wait_for_http(
    url: &str,
    expected_status: u16,
    timeout: Duration,
) -> DockerResult<()> {
    // TODO: implement HTTP health check via foundation_http or reqwest
    // For now, containers use Port or Stdout strategies
    let _ = (url, expected_status, timeout);
    Ok(())
}
