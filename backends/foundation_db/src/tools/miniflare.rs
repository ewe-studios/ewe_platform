//! Miniflare subprocess harness for Cloudflare Workers local testing.
//!
//! `Miniflare` starts a `wrangler dev --local` (or `npx miniflare`) child
//! process with configurable D1 databases, R2 buckets, and worker script,
//! waits for the HTTP endpoint, and kills it on drop.
//!
//! ```rust,no_run
//! use foundation_db::tools::Miniflare;
//!
//! let mf = Miniflare::builder()
//!     .worker_dir("examples/cf-login-app")
//!     .d1_database("DB")
//!     .r2_bucket("STORAGE")
//!     .build()
//!     .expect("failed to start miniflare");
//!
//! println!("Worker at {}", mf.base_url());
//! // process killed when `mf` is dropped
//! ```

use std::io::Read as IoRead;
use std::io::Write as IoWrite;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// A running miniflare/wrangler-dev process.
///
/// Kills the child process on drop. Use [`base_url()`](Self::base_url)
/// to get the HTTP endpoint for requests.
pub struct Miniflare {
    child: Option<Child>,
    pub port: u16,
}

impl Miniflare {
    /// Start with the default configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if wrangler/miniflare is not on PATH, or if the
    /// process doesn't become ready within the timeout.
    pub fn start() -> Result<Self, MiniflareError> {
        Self::start_with(MiniflareConfig::default())
    }

    /// Start with explicit configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the tool binary is missing, the worker directory
    /// doesn't exist, or the process doesn't become ready within the timeout.
    pub fn start_with(config: MiniflareConfig) -> Result<Self, MiniflareError> {
        if let Some(ref dir) = config.worker_dir {
            if !dir.exists() {
                return Err(MiniflareError::WorkerDirNotFound(
                    dir.display().to_string(),
                ));
            }
        }

        let tool = resolve_tool(&config.tool)?;
        let port = config.port.unwrap_or_else(|| {
            portpicker::pick_unused_port().expect("no free port available")
        });

        let mut args = build_args(&config, &tool, port);

        tracing::info!(
            port,
            tool = %tool.display(),
            "starting miniflare"
        );

        let mut cmd = Command::new(&tool);

        if tool.to_string_lossy().contains("npx") {
            cmd.arg(args.remove(0));
        }

        cmd.args(&args);

        if let Some(ref dir) = config.worker_dir {
            cmd.current_dir(dir);
        }

        let child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(MiniflareError::SpawnFailed)?;

        let pid = child.id();
        tracing::info!(pid, "miniflare spawned, waiting for readiness");

        let mut server = Self {
            child: Some(child),
            port,
        };

        if !wait_for_ready(port, config.startup_timeout) {
            let stderr = server.collect_stderr();
            server.stop();
            return Err(MiniflareError::ReadyTimeout {
                seconds: config.startup_timeout.as_secs(),
                stderr,
            });
        }

        tracing::info!(pid, port, "miniflare ready");
        Ok(server)
    }

    /// Return a builder for fine-grained configuration.
    #[must_use]
    pub fn builder() -> MiniflareConfig {
        MiniflareConfig::default()
    }

    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Send a GET request to the worker and return (status_code, body).
    ///
    /// # Errors
    ///
    /// Returns an error if the TCP connection or HTTP exchange fails.
    pub fn get(&self, path: &str) -> Result<(u16, String), MiniflareError> {
        http_get("127.0.0.1", self.port, path)
    }

    /// Gracefully stop the process without waiting for drop.
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            tracing::info!(pid = child.id(), "stopping miniflare");
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

impl Drop for Miniflare {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Which tool to use for local Workers emulation.
#[derive(Debug, Clone)]
pub enum MinflareTool {
    /// `wrangler dev` (default — most compatible, uses miniflare internally).
    Wrangler,
    /// Standalone `miniflare` CLI.
    Miniflare,
    /// Explicit path to a binary.
    Custom(PathBuf),
}

impl Default for MinflareTool {
    fn default() -> Self {
        Self::Wrangler
    }
}

/// Configuration for a miniflare instance.
///
/// Use [`Miniflare::builder()`] to get a default config, then chain
/// setters: `.worker_dir("...").d1_database("DB").build()`.
pub struct MiniflareConfig {
    pub tool: MinflareTool,
    pub port: Option<u16>,
    pub worker_dir: Option<PathBuf>,
    pub d1_databases: Vec<String>,
    pub r2_buckets: Vec<String>,
    pub kv_namespaces: Vec<String>,
    pub compatibility_date: String,
    pub startup_timeout: Duration,
    pub extra_args: Vec<String>,
}

impl Default for MiniflareConfig {
    fn default() -> Self {
        Self {
            tool: MinflareTool::default(),
            port: None,
            worker_dir: None,
            d1_databases: Vec::new(),
            r2_buckets: Vec::new(),
            kv_namespaces: Vec::new(),
            compatibility_date: "2024-01-01".into(),
            startup_timeout: Duration::from_secs(30),
            extra_args: Vec::new(),
        }
    }
}

impl MiniflareConfig {
    /// Fixed port (default: auto-pick a free port).
    #[must_use]
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Directory containing the worker script and wrangler.toml.
    #[must_use]
    pub fn worker_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.worker_dir = Some(path.into());
        self
    }

    /// Add a D1 database binding name (e.g. "DB").
    #[must_use]
    pub fn d1_database(mut self, name: impl Into<String>) -> Self {
        self.d1_databases.push(name.into());
        self
    }

    /// Add an R2 bucket binding name (e.g. "STORAGE").
    #[must_use]
    pub fn r2_bucket(mut self, name: impl Into<String>) -> Self {
        self.r2_buckets.push(name.into());
        self
    }

    /// Add a KV namespace binding name.
    #[must_use]
    pub fn kv_namespace(mut self, name: impl Into<String>) -> Self {
        self.kv_namespaces.push(name.into());
        self
    }

    /// Which tool to use (default: wrangler).
    #[must_use]
    pub fn tool(mut self, tool: MinflareTool) -> Self {
        self.tool = tool;
        self
    }

    /// Compatibility date for the Workers runtime.
    #[must_use]
    pub fn compatibility_date(mut self, date: impl Into<String>) -> Self {
        self.compatibility_date = date.into();
        self
    }

    #[must_use]
    pub fn startup_timeout(mut self, d: Duration) -> Self {
        self.startup_timeout = d;
        self
    }

    /// Add extra CLI arguments passed directly to the tool.
    #[must_use]
    pub fn extra_arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(arg.into());
        self
    }

    /// Build and start the miniflare process.
    ///
    /// # Errors
    ///
    /// Returns an error if the tool is missing, the worker dir doesn't exist,
    /// or the process doesn't become ready within the timeout.
    pub fn build(self) -> Result<Miniflare, MiniflareError> {
        Miniflare::start_with(self)
    }
}

/// Errors from the miniflare harness.
#[derive(Debug, derive_more::Display, derive_more::Error)]
pub enum MiniflareError {
    #[display(
        "neither wrangler nor miniflare found on PATH.\n\
        Install: npm install -g wrangler"
    )]
    #[error(ignore)]
    ToolNotFound,

    #[display("worker directory not found: {_0}")]
    #[error(ignore)]
    WorkerDirNotFound(String),

    #[display("failed to spawn miniflare: {_0}")]
    SpawnFailed(std::io::Error),

    #[display(
        "miniflare did not become ready within {seconds} seconds\nstderr:\n{stderr}"
    )]
    #[error(ignore)]
    ReadyTimeout { seconds: u64, stderr: String },

    #[display("HTTP request failed: {_0}")]
    #[error(ignore)]
    HttpFailed(String),
}

fn resolve_tool(tool: &MinflareTool) -> Result<PathBuf, MiniflareError> {
    match tool {
        MinflareTool::Custom(path) => Ok(path.clone()),
        MinflareTool::Wrangler => which::which("wrangler")
            .or_else(|_| which::which("npx").map(|p| p))
            .map_err(|_| MiniflareError::ToolNotFound),
        MinflareTool::Miniflare => which::which("miniflare")
            .or_else(|_| which::which("npx"))
            .map_err(|_| MiniflareError::ToolNotFound),
    }
}

fn build_args(config: &MiniflareConfig, tool_path: &Path, port: u16) -> Vec<String> {
    let tool_name = tool_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let is_npx = tool_name == "npx";

    let mut args: Vec<String> = Vec::new();

    match &config.tool {
        MinflareTool::Wrangler => {
            if is_npx {
                args.push("wrangler".into());
            }
            args.extend(["dev".into(), "--local".into()]);
            args.extend(["--port".into(), port.to_string()]);
            args.extend(["--ip".into(), "127.0.0.1".into()]);
        }
        MinflareTool::Miniflare => {
            if is_npx {
                args.push("miniflare".into());
            }
            args.extend(["--port".into(), port.to_string()]);
            args.extend(["--host".into(), "127.0.0.1".into()]);
            args.push("--modules".into());
        }
        MinflareTool::Custom(_) => {
            args.extend(["--port".into(), port.to_string()]);
            args.extend(["--host".into(), "127.0.0.1".into()]);
        }
    }

    for db in &config.d1_databases {
        match &config.tool {
            MinflareTool::Miniflare => {
                args.extend([
                    "--d1".into(),
                    format!("{db}={db}"),
                ]);
            }
            _ => {}
        }
    }

    for bucket in &config.r2_buckets {
        match &config.tool {
            MinflareTool::Miniflare => {
                args.extend([
                    "--r2".into(),
                    format!("{bucket}={bucket}"),
                ]);
            }
            _ => {}
        }
    }

    for ns in &config.kv_namespaces {
        match &config.tool {
            MinflareTool::Miniflare => {
                args.extend(["--kv".into(), format!("{ns}={ns}")]);
            }
            _ => {}
        }
    }

    args.extend(config.extra_args.iter().cloned());
    args
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

fn http_get(host: &str, port: u16, path: &str) -> Result<(u16, String), MiniflareError> {
    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect(&addr)
        .map_err(|e| MiniflareError::HttpFailed(format!("connect {addr}: {e}")))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| MiniflareError::HttpFailed(format!("set timeout: {e}")))?;

    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| MiniflareError::HttpFailed(format!("write: {e}")))?;

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
