//! Cloudflare `workerd` subprocess harness.
//!
//! `Workerd` starts a `workerd serve` child process from a Cap'n Proto config
//! file, waits for the HTTP endpoint, and kills it on drop.
//!
//! ```rust,no_run
//! use foundation_toolings::harness::Workerd;
//!
//! let wd = Workerd::builder()
//!     .config_file("worker.capnp")
//!     .build()
//!     .expect("failed to start workerd");
//!
//! println!("Worker at {}", wd.base_url());
//! // process killed when `wd` is dropped
//! ```

use std::io::Read as IoRead;
use std::io::Write as IoWrite;
use std::net::TcpStream;
use std::path::{PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct Workerd {
    child: Option<Child>,
    pub port: u16,
}

impl Workerd {
    pub fn start() -> Result<Self, WorkerdError> {
        Self::start_with(WorkerdConfig::default())
    }

    pub fn start_with(config: WorkerdConfig) -> Result<Self, WorkerdError> {
        let config_file = config
            .config_file
            .as_ref()
            .ok_or(WorkerdError::NoConfigFile)?;

        if !config_file.exists() {
            return Err(WorkerdError::ConfigNotFound(
                config_file.display().to_string(),
            ));
        }

        let bin = resolve_bin(&config.binary)?;
        let port = config.port.unwrap_or_else(|| {
            super::pick_unused_port().expect("no free port available")
        });

        tracing::info!(
            port,
            config = %config_file.display(),
            bin = %bin.display(),
            "starting workerd"
        );

        let mut cmd = Command::new(&bin);
        cmd.arg("serve");
        cmd.arg(config_file);

        cmd.arg("--socket-addr");
        cmd.arg(format!("127.0.0.1:{port}"));

        if config.verbose {
            cmd.arg("--verbose");
        }

        for arg in &config.extra_args {
            cmd.arg(arg);
        }

        if let Some(ref dir) = config.working_dir {
            cmd.current_dir(dir);
        }

        let child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(WorkerdError::SpawnFailed)?;

        let pid = child.id();
        tracing::info!(pid, "workerd spawned, waiting for readiness");

        let mut server = Self {
            child: Some(child),
            port,
        };

        if !wait_for_ready(port, config.startup_timeout) {
            let stderr = server.collect_stderr();
            server.stop();
            return Err(WorkerdError::ReadyTimeout {
                seconds: config.startup_timeout.as_secs(),
                stderr,
            });
        }

        tracing::info!(pid, port, "workerd ready");
        Ok(server)
    }

    #[must_use]
    pub fn builder() -> WorkerdConfig {
        WorkerdConfig::default()
    }

    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn get(&self, path: &str) -> Result<(u16, String), WorkerdError> {
        http_get("127.0.0.1", self.port, path)
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            tracing::info!(pid = child.id(), "stopping workerd");
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn collect_stderr(&mut self) -> String {
        let Some(ref mut child) = self.child else {
            return String::new();
        };
        let Some(mut stderr) = child.stderr.take() else {
            return String::new();
        };
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        buf
    }
}

impl Drop for Workerd {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct WorkerdConfig {
    pub binary: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub port: Option<u16>,
    pub working_dir: Option<PathBuf>,
    pub verbose: bool,
    pub startup_timeout: Duration,
    pub extra_args: Vec<String>,
}

impl Default for WorkerdConfig {
    fn default() -> Self {
        Self {
            binary: None,
            config_file: None,
            port: None,
            working_dir: None,
            verbose: false,
            startup_timeout: Duration::from_secs(15),
            extra_args: Vec::new(),
        }
    }
}

impl WorkerdConfig {
    #[must_use]
    pub fn binary(mut self, path: impl Into<PathBuf>) -> Self {
        self.binary = Some(path.into());
        self
    }

    #[must_use]
    pub fn config_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_file = Some(path.into());
        self
    }

    #[must_use]
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    #[must_use]
    pub fn working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(path.into());
        self
    }

    #[must_use]
    pub fn verbose(mut self, v: bool) -> Self {
        self.verbose = v;
        self
    }

    #[must_use]
    pub fn startup_timeout(mut self, d: Duration) -> Self {
        self.startup_timeout = d;
        self
    }

    #[must_use]
    pub fn extra_arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(arg.into());
        self
    }

    pub fn build(self) -> Result<Workerd, WorkerdError> {
        Workerd::start_with(self)
    }
}

#[derive(Debug, derive_more::Display, derive_more::Error)]
pub enum WorkerdError {
    #[display("no config file specified — use .config_file(\"worker.capnp\")")]
    #[error(ignore)]
    NoConfigFile,

    #[display("workerd binary not found on PATH.\nInstall: https://github.com/cloudflare/workerd")]
    #[error(ignore)]
    BinaryNotFound,

    #[display("config file not found: {_0}")]
    #[error(ignore)]
    ConfigNotFound(String),

    #[display("failed to spawn workerd: {_0}")]
    SpawnFailed(std::io::Error),

    #[display(
        "workerd did not become ready within {seconds} seconds\nstderr:\n{stderr}"
    )]
    #[error(ignore)]
    ReadyTimeout { seconds: u64, stderr: String },

    #[display("HTTP request failed: {_0}")]
    #[error(ignore)]
    HttpFailed(String),
}

fn resolve_bin(explicit: &Option<PathBuf>) -> Result<PathBuf, WorkerdError> {
    if let Some(path) = explicit {
        return Ok(path.clone());
    }
    which::which("workerd").map_err(|_| WorkerdError::BinaryNotFound)
}

fn wait_for_ready(port: u16, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if http_get("127.0.0.1", port, "/").is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

fn http_get(host: &str, port: u16, path: &str) -> Result<(u16, String), WorkerdError> {
    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect(&addr)
        .map_err(|e| WorkerdError::HttpFailed(format!("connect {addr}: {e}")))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| WorkerdError::HttpFailed(format!("set timeout: {e}")))?;

    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| WorkerdError::HttpFailed(format!("write: {e}")))?;

    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);

    let status_code = response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);

    let body = response
        .split_once("\r\n\r\n")
        .map_or(response.as_str(), |(_, body)| body)
        .to_string();

    Ok((status_code, body))
}
