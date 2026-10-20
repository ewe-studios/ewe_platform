//! Miniflare subprocess harness for Cloudflare Workers local testing.
//!
//! `Miniflare` starts a `wrangler dev --local` (or `npx miniflare`) child
//! process with configurable D1 databases, R2 buckets, and worker script,
//! waits for the HTTP endpoint, and kills it on drop.
//!
//! ```rust,no_run
//! use foundation_toolings::harness::Miniflare;
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

use std::collections::VecDeque;
use std::io::Read as IoRead;
use std::io::Write as IoWrite;
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The last lines a child wrote to one of its pipes, capped at `cap` lines.
#[derive(Clone)]
struct LogTail {
    lines: Arc<Mutex<VecDeque<String>>>,
    cap: usize,
}

impl LogTail {
    fn new(cap: usize) -> Self {
        Self {
            lines: Arc::new(Mutex::new(VecDeque::with_capacity(cap.min(1024)))),
            cap,
        }
    }

    fn push(&self, line: String) {
        let mut lines = self
            .lines
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if lines.len() == self.cap {
            lines.pop_front();
        }
        if self.cap > 0 {
            lines.push_back(line);
        }
    }

    fn joined(&self) -> String {
        let lines = self
            .lines
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        lines.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

/// Drain `pipe` line by line on a background thread into `tail`.
///
/// WHY: a piped child that nobody reads blocks once the OS pipe buffer fills;
/// wrangler logs every request, so an undrained pipe stalls the worker mid-test.
fn drain<R: std::io::Read + Send + 'static>(pipe: R, tail: LogTail) -> JoinHandle<()> {
    std::thread::spawn(move || {
        for line in BufReader::new(pipe).lines() {
            match line {
                Ok(line) => tail.push(line),
                Err(_) => break,
            }
        }
    })
}

pub struct Miniflare {
    child: Option<Child>,
    pub port: u16,
    stdout: LogTail,
    stderr: LogTail,
    drains: Vec<JoinHandle<()>>,
}

impl Miniflare {
    pub fn start() -> Result<Self, MiniflareError> {
        Self::start_with(MiniflareConfig::default())
    }

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
            super::pick_unused_port().expect("no free port available")
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

        for (name, value) in &config.vars {
            // `wrangler dev --var NAME:VALUE` (the value may itself contain `:`).
            if matches!(config.tool, MinflareTool::Wrangler) {
                cmd.arg("--var").arg(format!("{name}:{value}"));
            }
        }

        // Own process group, so `stop` can take down wrangler's workerd
        // children too: killing only the direct child leaves them running,
        // holding our pipes open (and leaking a workerd per run).
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(MiniflareError::SpawnFailed)?;

        let pid = child.id();
        tracing::info!(pid, "miniflare spawned, waiting for readiness");

        let stdout = LogTail::new(config.log_tail_lines);
        let stderr = LogTail::new(config.log_tail_lines);
        let mut drains = Vec::new();
        if let Some(pipe) = child.stdout.take() {
            drains.push(drain(pipe, stdout.clone()));
        }
        if let Some(pipe) = child.stderr.take() {
            drains.push(drain(pipe, stderr.clone()));
        }

        let mut server = Self {
            child: Some(child),
            port,
            stdout,
            stderr,
            drains,
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

    #[must_use]
    pub fn builder() -> MiniflareConfig {
        MiniflareConfig::default()
    }

    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn get(&self, path: &str) -> Result<(u16, String), MiniflareError> {
        http_get("127.0.0.1", self.port, path)
    }

    /// `POST body` to `path` and read the whole response (`Connection: close`).
    ///
    /// Test helper: it waits up to `timeout` for the worker to finish the
    /// response, so a streamed body arrives complete.
    pub fn post(
        &self,
        path: &str,
        body: &str,
        timeout: Duration,
    ) -> Result<(u16, String), MiniflareError> {
        http_request("127.0.0.1", self.port, "POST", path, body, timeout)
    }

    /// The last lines the worker process wrote to stdout (see
    /// [`MiniflareConfig::log_tail_lines`]).
    #[must_use]
    pub fn stdout_tail(&self) -> String {
        self.stdout.joined()
    }

    /// The last lines the worker process wrote to stderr.
    #[must_use]
    pub fn stderr_tail(&self) -> String {
        self.stderr.joined()
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let pid = child.id();
            tracing::info!(pid, "stopping miniflare");
            #[cfg(unix)]
            {
                // The child leads its own process group (see `start_with`).
                if let Ok(pgid) = libc::pid_t::try_from(pid) {
                    // SAFETY: `kill` takes plain integers and has no memory
                    // effects; a negative pid addresses the process group.
                    let rc = unsafe { libc::kill(-pgid, libc::SIGKILL) };
                    if rc != 0 {
                        tracing::warn!(
                            pid,
                            error = %std::io::Error::last_os_error(),
                            "killing the miniflare process group failed"
                        );
                    }
                }
            }
            if let Err(e) = child.kill() {
                // Already gone after the group kill: expected.
                tracing::debug!(pid, error = %e, "miniflare child already exited");
            }
            if let Err(e) = child.wait() {
                tracing::warn!(pid, error = %e, "reaping the miniflare child failed");
            }
        }
        // The drain threads end on their own once every writer of the pipes is
        // gone; never block shutdown on them (a stray grandchild that escaped
        // the process group would otherwise hang the caller here).
        self.drains.clear();
    }

    fn collect_stderr(&mut self) -> String {
        self.stderr_tail()
    }
}

impl Drop for Miniflare {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug, Clone)]
pub enum MinflareTool {
    Wrangler,
    Miniflare,
    Custom(PathBuf),
}

impl Default for MinflareTool {
    fn default() -> Self {
        Self::Wrangler
    }
}

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
    /// Worker vars passed as `wrangler dev --var NAME:VALUE` (wrangler only).
    pub vars: Vec<(String, String)>,
    /// How many of the child's most recent stdout/stderr lines to keep for
    /// error reports. Both pipes are always drained; this only bounds memory.
    /// Default 200, about a screenful of wrangler output.
    pub log_tail_lines: usize,
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
            vars: Vec::new(),
            log_tail_lines: 200,
        }
    }
}

impl MiniflareConfig {
    #[must_use]
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    #[must_use]
    pub fn worker_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.worker_dir = Some(path.into());
        self
    }

    #[must_use]
    pub fn d1_database(mut self, name: impl Into<String>) -> Self {
        self.d1_databases.push(name.into());
        self
    }

    #[must_use]
    pub fn r2_bucket(mut self, name: impl Into<String>) -> Self {
        self.r2_buckets.push(name.into());
        self
    }

    #[must_use]
    pub fn kv_namespace(mut self, name: impl Into<String>) -> Self {
        self.kv_namespaces.push(name.into());
        self
    }

    #[must_use]
    pub fn tool(mut self, tool: MinflareTool) -> Self {
        self.tool = tool;
        self
    }

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

    /// Set a worker var (`wrangler dev --var NAME:VALUE`). The examples read
    /// their secrets with a var fallback, so this also stands in for secrets in
    /// local runs.
    #[must_use]
    pub fn var(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.vars.push((name.into(), value.into()));
        self
    }

    /// See [`MiniflareConfig::log_tail_lines`].
    #[must_use]
    pub fn log_tail_lines(mut self, lines: usize) -> Self {
        self.log_tail_lines = lines;
        self
    }

    #[must_use]
    pub fn extra_arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(arg.into());
        self
    }

    pub fn build(self) -> Result<Miniflare, MiniflareError> {
        Miniflare::start_with(self)
    }
}

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
            .or_else(|_| which::which("npx"))
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
        if let MinflareTool::Miniflare = &config.tool {
            args.extend(["--d1".into(), format!("{db}={db}")]);
        }
    }

    for bucket in &config.r2_buckets {
        if let MinflareTool::Miniflare = &config.tool {
            args.extend(["--r2".into(), format!("{bucket}={bucket}")]);
        }
    }

    for ns in &config.kv_namespaces {
        if let MinflareTool::Miniflare = &config.tool {
            args.extend(["--kv".into(), format!("{ns}={ns}")]);
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
    http_request(host, port, "GET", path, "", Duration::from_secs(2))
}

fn http_request(
    host: &str,
    port: u16,
    method: &str,
    path: &str,
    body: &str,
    timeout: Duration,
) -> Result<(u16, String), MiniflareError> {
    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect(&addr)
        .map_err(|e| MiniflareError::HttpFailed(format!("connect {addr}: {e}")))?;

    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| MiniflareError::HttpFailed(format!("set timeout: {e}")))?;

    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\
         Content-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| MiniflareError::HttpFailed(format!("write: {e}")))?;

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| MiniflareError::HttpFailed(format!("read {method} {path}: {e}")))?;
    let response = String::from_utf8_lossy(&raw);

    let status_code = response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);

    let (head, body) = response
        .split_once("\r\n\r\n")
        .unwrap_or((response.as_ref(), ""));
    let chunked = head.lines().any(|line| {
        let line = line.to_ascii_lowercase();
        line.starts_with("transfer-encoding:") && line.contains("chunked")
    });
    let body = if chunked {
        dechunk(body)?
    } else {
        body.to_string()
    };

    Ok((status_code, body))
}

/// Decode an HTTP/1.1 `Transfer-Encoding: chunked` body.
fn dechunk(mut body: &str) -> Result<String, MiniflareError> {
    let mut out = String::new();
    loop {
        let (size_line, rest) = body
            .split_once("\r\n")
            .ok_or_else(|| MiniflareError::HttpFailed("truncated chunk header".into()))?;
        let size_hex = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_hex, 16)
            .map_err(|e| MiniflareError::HttpFailed(format!("bad chunk size {size_hex:?}: {e}")))?;
        if size == 0 {
            return Ok(out);
        }
        let chunk = rest
            .get(..size)
            .ok_or_else(|| MiniflareError::HttpFailed("truncated chunk".into()))?;
        out.push_str(chunk);
        body = rest
            .get(size..)
            .and_then(|r| r.strip_prefix("\r\n"))
            .ok_or_else(|| MiniflareError::HttpFailed("missing chunk terminator".into()))?;
    }
}
