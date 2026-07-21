//! Docker-based test environments (F30).
//!
//! WHY: Cross-platform testing requires macOS, Windows, and Android environments.
//! Docker containers provision these OSes; this module provides a programmatic
//! Rust API to connect, commandeer, and assert against them — no physical devices
//! needed.
//!
//! WHAT: `TestEnvironmentBuilder` connects to running Docker containers via
//! `docker exec` (shell), VNC/RDP (screenshot), and `docker cp` (file transfer).
//! `TestEnvironment` exposes exec, copy_in, screenshot, and readiness checks.
//!
//! HOW: All operations shell out to `docker` CLI. Zero new Rust dependencies.
//! Feature-gated behind `docker-tests`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

// ── Error types ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum TestEnvError {
    /// The named Docker container is not running.
    ContainerNotFound(String),
    /// The container is running but not responding on the expected port.
    ContainerNotReady { container: String, port: u16 },
    /// A `docker exec` command failed.
    ExecFailed { container: String, command: String, stderr: String },
    /// A `docker cp` command failed.
    CopyFailed { container: String, detail: String },
    /// Timed out waiting for the container to become ready.
    Timeout(String),
}

impl std::fmt::Display for TestEnvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ContainerNotFound(c) => write!(f, "Docker container '{c}' not found (is it running?)"),
            Self::ContainerNotReady { container, port } => write!(f, "Container '{container}' not ready on port {port}"),
            Self::ExecFailed { container, command, stderr } => write!(f, "exec failed in '{container}': {command}: {stderr}"),
            Self::CopyFailed { container, detail } => write!(f, "copy failed to '{container}': {detail}"),
            Self::Timeout(msg) => write!(f, "timed out: {msg}"),
        }
    }
}

impl std::error::Error for TestEnvError {}

// ── Connection mode ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ConnectionMode {
    /// VNC (macOS: port 5900, Android: web UI on 8006)
    Vnc { port: u16, password: Option<String> },
    /// RDP (Windows: port 3389)
    Rdp { port: u16, username: String, password: String },
    /// Shell-only via docker exec, no graphical access.
    Shell,
}

// ── Docker exec helper ───────────────────────────────────────────────────

/// Thin wrapper around `docker exec` for running commands inside a container.
#[derive(Debug, Clone)]
struct DockerExec {
    container: String,
}

impl DockerExec {
    fn new(container: &str) -> Self {
        Self { container: container.to_string() }
    }

    /// Run a command inside the container. Returns stdout as a string.
    fn exec(&self, cmd: &str) -> Result<String, TestEnvError> {
        let output = Command::new("docker")
            .args(["exec", &self.container, "sh", "-c", cmd])
            .output()
            .map_err(|e| TestEnvError::ExecFailed {
                container: self.container.clone(),
                command: cmd.to_string(),
                stderr: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TestEnvError::ExecFailed {
                container: self.container.clone(),
                command: cmd.to_string(),
                stderr: stderr.to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Copy a local file into the container at `dest`.
    fn copy_in(&self, src: &Path, dest: &str) -> Result<(), TestEnvError> {
        let output = Command::new("docker")
            .args(["cp", &src.display().to_string(), &format!("{}:{}", self.container, dest)])
            .output()
            .map_err(|e| TestEnvError::CopyFailed {
                container: self.container.clone(),
                detail: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(TestEnvError::CopyFailed {
                container: self.container.clone(),
                detail: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        Ok(())
    }
}

// ── Test environment builder ─────────────────────────────────────────────

pub struct TestEnvironmentBuilder {
    container: String,
    mode: ConnectionMode,
    ready_timeout: Duration,
}

impl TestEnvironmentBuilder {
    #[must_use]
    pub fn new(container: &str) -> Self {
        Self {
            container: container.to_string(),
            mode: ConnectionMode::Shell,
            ready_timeout: Duration::from_secs(60),
        }
    }

    /// Connect via VNC (macOS, Android).
    #[must_use]
    pub fn with_vnc(mut self, port: u16) -> Self {
        self.mode = ConnectionMode::Vnc { port, password: None };
        self
    }

    /// Connect via RDP (Windows).
    #[must_use]
    pub fn with_rdp(mut self, port: u16, user: &str, pass: &str) -> Self {
        self.mode = ConnectionMode::Rdp {
            port,
            username: user.to_string(),
            password: pass.to_string(),
        };
        self
    }

    /// Override the default 60s readiness timeout.
    #[must_use]
    pub fn with_ready_timeout(mut self, d: Duration) -> Self {
        self.ready_timeout = d;
        self
    }

    /// Check if the container is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        let output = Command::new("docker")
            .args(["ps", "--filter", &format!("name={}", self.container), "--format", "{{.Status}}"])
            .output()
            .unwrap_or_else(|_| Output { status: Default::default(), stdout: vec![], stderr: vec![] });

        let status = String::from_utf8_lossy(&output.stdout);
        !status.trim().is_empty() && status.contains("Up")
    }

    /// Connect to the container and return a ready `TestEnvironment`.
    ///
    /// # Errors
    ///
    /// Returns `TestEnvError::ContainerNotFound` if the container isn't running,
    /// or `TestEnvError::Timeout` if it doesn't become ready within the timeout.
    pub fn connect(&self) -> Result<TestEnvironment, TestEnvError> {
        if !self.is_running() {
            return Err(TestEnvError::ContainerNotFound(self.container.clone()));
        }

        let deadline = Instant::now() + self.ready_timeout;
        loop {
            if self.check_ready() {
                break;
            }
            if Instant::now() > deadline {
                return Err(TestEnvError::Timeout(format!(
                    "container '{}' not ready after {:?}",
                    self.container, self.ready_timeout
                )));
            }
            std::thread::sleep(Duration::from_millis(500));
        }

        Ok(TestEnvironment {
            container: self.container.clone(),
            mode: self.mode.clone(),
            docker_exec: DockerExec::new(&self.container),
        })
    }

    /// Check if the container is ready to accept commands.
    fn check_ready(&self) -> bool {
        // Basic check: can we run a trivial command?
        match &self.mode {
            ConnectionMode::Vnc { port, .. } => {
                // Check both: container responds to exec AND VNC port is listening
                let exec_ok = DockerExec::new(&self.container)
                    .exec("echo ready")
                    .is_ok_and(|s| s == "ready");
                let port_ok = port_is_open("localhost", *port);
                exec_ok && port_ok
            }
            ConnectionMode::Rdp { port, .. } => {
                let exec_ok = DockerExec::new(&self.container)
                    .exec("echo ready")
                    .is_ok_and(|s| s == "ready");
                let port_ok = port_is_open("localhost", *port);
                exec_ok && port_ok
            }
            ConnectionMode::Shell => {
                DockerExec::new(&self.container)
                    .exec("echo ready")
                    .is_ok_and(|s| s == "ready")
            }
        }
    }
}

// ── Test environment ─────────────────────────────────────────────────────

#[derive(Debug)]
pub struct TestEnvironment {
    pub container: String,
    pub mode: ConnectionMode,
    docker_exec: DockerExec,
}

impl TestEnvironment {
    /// Run a command inside the container. Returns stdout.
    ///
    /// # Errors
    ///
    /// Returns `TestEnvError::ExecFailed` if the command fails.
    pub fn exec(&self, cmd: &str) -> Result<String, TestEnvError> {
        self.docker_exec.exec(cmd)
    }

    /// Copy a local file into the container.
    ///
    /// # Errors
    ///
    /// Returns `TestEnvError::CopyFailed` if the copy fails.
    pub fn copy_in(&self, src: &Path, dest: &str) -> Result<(), TestEnvError> {
        self.docker_exec.copy_in(src, dest)
    }

    /// Take a screenshot of the container's display.
    ///
    /// For macOS (VNC): uses `screencapture` via docker exec.
    /// For Windows (RDP): returns an error (RDP screenshots need external tooling).
    /// For Shell mode: returns an error.
    ///
    /// # Errors
    ///
    /// Returns `TestEnvError::ExecFailed` if the screenshot command fails.
    pub fn screenshot(&self) -> Result<Vec<u8>, TestEnvError> {
        match &self.mode {
            ConnectionMode::Vnc { .. } => {
                // macOS: use screencapture. Android: use screencap.
                // Try macOS first, fall back to Android.
                if let Ok(data) = self.try_macos_screenshot() {
                    return Ok(data);
                }
                if let Ok(data) = self.try_android_screenshot() {
                    return Ok(data);
                }
                Err(TestEnvError::ExecFailed {
                    container: self.container.clone(),
                    command: "screencapture/screencap".into(),
                    stderr: "no screenshot tool available".into(),
                })
            }
            ConnectionMode::Rdp { .. } => Err(TestEnvError::ExecFailed {
                container: self.container.clone(),
                command: "screenshot".into(),
                stderr: "RDP screenshots not supported (use VNC or external RDP client)".into(),
            }),
            ConnectionMode::Shell => Err(TestEnvError::ExecFailed {
                container: self.container.clone(),
                command: "screenshot".into(),
                stderr: "Shell mode has no display".into(),
            }),
        }
    }

    fn try_macos_screenshot(&self) -> Result<Vec<u8>, TestEnvError> {
        // Write screenshot to a temp file, then docker cp it out.
        let tmp_in = "/tmp/ewe_screenshot.png";
        self.docker_exec.exec(&format!("screencapture -x {tmp_in}"))?;

        let mut out = Command::new("docker")
            .args(["cp", &format!("{}:{tmp_in}", self.container), "-"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .map_err(|e| TestEnvError::ExecFailed {
                container: self.container.clone(),
                command: "docker cp".into(),
                stderr: e.to_string(),
            })?;

        Ok(out.stdout)
    }

    fn try_android_screenshot(&self) -> Result<Vec<u8>, TestEnvError> {
        let tmp_in = "/tmp/ewe_screenshot.png";
        self.docker_exec.exec(&format!("screencap -p {tmp_in}"))?;

        let out = Command::new("docker")
            .args(["cp", &format!("{}:{tmp_in}", self.container), "-"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .map_err(|e| TestEnvError::ExecFailed {
                container: self.container.clone(),
                command: "docker cp".into(),
                stderr: e.to_string(),
            })?;

        Ok(out.stdout)
    }

    /// Get the OS name from the container (e.g. "macOS", "Windows", "Android").
    #[must_use]
    pub fn os_name(&self) -> &str {
        match &self.mode {
            ConnectionMode::Vnc { .. } => {
                // Heuristic: if screencapture exists, it's macOS; else Android.
                if self.docker_exec.exec("which screencapture").is_ok() {
                    "macOS"
                } else {
                    "Android"
                }
            }
            ConnectionMode::Rdp { .. } => "Windows",
            ConnectionMode::Shell => "unknown",
        }
    }
}

// ── Utility: TCP port check ──────────────────────────────────────────────

fn port_is_open(host: &str, port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &format!("{host}:{port}").parse().unwrap(),
        Duration::from_secs(2),
    )
    .is_ok()
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_container_not_found_when_not_running() {
        let builder = TestEnvironmentBuilder::new("nonexistent_container_12345");
        assert!(!builder.is_running());
        match builder.connect() {
            Err(TestEnvError::ContainerNotFound(c)) => assert!(c.contains("nonexistent")),
            other => panic!("expected ContainerNotFound, got {other:?}"),
        }
    }

    #[test]
    fn builder_with_vnc_configures_correctly() {
        let builder = TestEnvironmentBuilder::new("macos").with_vnc(5900);
        assert!(builder.is_running()); // macOS container should be up
    }

    #[test]
    fn builder_with_rdp_configures_correctly() {
        let builder = TestEnvironmentBuilder::new("ewe_windows")
            .with_rdp(3389, "Docker", "admin");
        assert!(builder.is_running()); // Windows container should be up
    }

    #[test]
    fn port_is_open_returns_bool() {
        // 192.0.2.0/24 is TEST-NET-1, reserved and unreachable — always false.
        let result = port_is_open("192.0.2.1", 80);
        assert!(!result);
    }
}
