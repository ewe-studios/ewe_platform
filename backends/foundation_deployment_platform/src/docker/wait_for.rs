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

use std::time::Duration;

/// A readiness condition for a running container.
#[derive(Debug, Clone)]
pub enum WaitFor {
    /// Wait for a TCP port to accept connections on the host.
    Port { port: u16, timeout: Duration },

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

    /// No wait — return as soon as the container reports "running".
    None,
}

impl Default for WaitFor {
    fn default() -> Self {
        Self::None
    }
}

impl WaitFor {
    /// Wait for a TCP port to be open. 30-second default timeout.
    #[must_use]
    pub fn port(port: u16) -> Self {
        Self::Port {
            port,
            timeout: Duration::from_secs(30),
        }
    }

    /// Wait for a TCP port with a custom timeout.
    #[must_use]
    pub fn port_with_timeout(port: u16, timeout: Duration) -> Self {
        Self::Port { port, timeout }
    }

    /// Wait for an HTTP 200 on the given URL. 30-second default timeout.
    #[must_use]
    pub fn http(url: impl Into<String>) -> Self {
        Self::Http {
            url: url.into(),
            expected_status: 200,
            timeout: Duration::from_secs(30),
        }
    }

    /// Wait for a stdout message. 30-second default timeout.
    #[must_use]
    pub fn stdout(message: impl Into<String>) -> Self {
        Self::Stdout {
            message: message.into(),
            timeout: Duration::from_secs(30),
        }
    }

    /// Combine multiple strategies — all must succeed in order.
    #[must_use]
    pub fn all(strategies: Vec<WaitFor>) -> Self {
        Self::Composite { strategies }
    }

    /// Returns the timeout duration or `None` for `WaitFor::None`.
    #[must_use]
    pub fn timeout(&self) -> Option<Duration> {
        match self {
            Self::Port { timeout, .. }
            | Self::Http { timeout, .. }
            | Self::Stdout { timeout, .. } => Some(*timeout),
            Self::Composite { strategies } => {
                strategies.iter().fold(Some(Duration::ZERO), |acc, s| {
                    match (acc, s.timeout()) {
                        (Some(a), Some(b)) => Some(a + b),
                        _ => acc,
                    }
                })
            }
            Self::None => None,
        }
    }
}
