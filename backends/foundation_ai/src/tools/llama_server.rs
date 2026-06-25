//! llama-server subprocess harness.
//!
//! `LlamaServer` starts a llama-server child process, waits for its `/health`
//! endpoint, and kills it on drop. Use in integration tests or dev tooling
//! instead of the external mise tasks.
//!
//! ```rust,no_run
//! use foundation_ai::tools::LlamaServer;
//!
//! let server = LlamaServer::builder()
//!     .model_file("path/to/model.gguf")
//!     .build()
//!     .expect("failed to start llama-server");
//!
//! println!("Server at {}", server.base_url());
//! // server is killed when `server` is dropped
//! ```

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn project_root() -> PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set by cargo");
    Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("foundation_ai should be two levels below project root")
        .to_path_buf()
}

/// A running llama-server process.
///
/// Kills the child process on drop. Access connection details via the
/// public fields and [`base_url()`](Self::base_url).
pub struct LlamaServer {
    child: Option<Child>,
    pub port: u16,
    pub api_key: String,
    pub model_name: String,
}

impl LlamaServer {
    /// Start with the default configuration (reads env vars, falls back to
    /// well-known paths).
    ///
    /// # Errors
    ///
    /// Returns an error if the server binary or model file is missing, or if
    /// the server doesn't become healthy within the timeout.
    pub fn start() -> Result<Self, LlamaServerError> {
        Self::start_with(LlamaServerConfig::default())
    }

    /// Start with explicit configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the server binary or model file is missing, or if
    /// the server doesn't become healthy within the timeout.
    pub fn start_with(config: LlamaServerConfig) -> Result<Self, LlamaServerError> {
        if !config.server_bin.exists() {
            return Err(LlamaServerError::BinaryNotFound(
                config.server_bin.display().to_string(),
            ));
        }
        if !config.model_file.exists() {
            return Err(LlamaServerError::ModelNotFound(
                config.model_file.display().to_string(),
            ));
        }

        let port = config.port.unwrap_or_else(|| {
            portpicker::pick_unused_port().expect("no free port available")
        });

        kill_existing(port);

        tracing::info!(
            port,
            model = %config.model_file.display(),
            "starting llama-server"
        );

        let child = Command::new(&config.server_bin)
            .args([
                "--model",
                config.model_file.to_str().unwrap_or_default(),
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--api-key",
                &config.api_key,
                "--threads",
                &config.threads.to_string(),
                "--ctx-size",
                &config.ctx_size.to_string(),
                "--batch-size",
                &config.batch_size.to_string(),
                "--ubatch-size",
                &config.ubatch_size.to_string(),
                "--no-webui",
                "--log-disable",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(LlamaServerError::SpawnFailed)?;

        let pid = child.id();
        tracing::info!(pid, "llama-server spawned, waiting for health check");

        if !wait_for_health(port, config.startup_timeout) {
            let guard = Self {
                child: Some(child),
                port,
                api_key: config.api_key,
                model_name: config.model_name,
            };
            drop(guard);
            return Err(LlamaServerError::HealthTimeout(
                config.startup_timeout.as_secs(),
            ));
        }

        tracing::info!(pid, port, "llama-server ready");

        Ok(Self {
            child: Some(child),
            port,
            api_key: config.api_key,
            model_name: config.model_name,
        })
    }

    /// Return a builder for fine-grained configuration.
    #[must_use]
    pub fn builder() -> LlamaServerConfig {
        LlamaServerConfig::default()
    }

    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Gracefully stop the server without waiting for drop.
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            tracing::info!(pid = child.id(), "stopping llama-server");
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for LlamaServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Configuration for a llama-server instance.
///
/// Use [`LlamaServer::builder()`] to get a default config, then chain
/// setters: `.port(8080).model_file("...").build()`.
pub struct LlamaServerConfig {
    pub port: Option<u16>,
    pub api_key: String,
    pub model_name: String,
    pub model_file: PathBuf,
    pub server_bin: PathBuf,
    pub threads: u32,
    pub ctx_size: u32,
    pub batch_size: u32,
    pub ubatch_size: u32,
    pub startup_timeout: Duration,
}

impl Default for LlamaServerConfig {
    fn default() -> Self {
        let root = project_root();
        Self {
            port: std::env::var("LLAMA_SERVER_PORT")
                .ok()
                .and_then(|p| p.parse().ok()),
            api_key: "test-api-key-123".into(),
            model_name: std::env::var("LLAMA_TEST_MODEL_NAME")
                .unwrap_or_else(|_| "qwen2.5-0.5b-instruct".into()),
            model_file: std::env::var("LLAMA_TEST_MODEL_FILE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    root.join("artefact/models/qwen2.5-0.5b-instruct-q4_k_m.gguf")
                }),
            server_bin: root.join("support/bin/llama-server"),
            threads: 4,
            ctx_size: 2048,
            batch_size: 512,
            ubatch_size: 256,
            startup_timeout: Duration::from_secs(60),
        }
    }
}

impl LlamaServerConfig {
    /// Fixed port (default: auto-pick a free port).
    #[must_use]
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    #[must_use]
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = key.into();
        self
    }

    #[must_use]
    pub fn model_name(mut self, name: impl Into<String>) -> Self {
        self.model_name = name.into();
        self
    }

    #[must_use]
    pub fn model_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.model_file = path.into();
        self
    }

    #[must_use]
    pub fn server_bin(mut self, path: impl Into<PathBuf>) -> Self {
        self.server_bin = path.into();
        self
    }

    #[must_use]
    pub fn threads(mut self, n: u32) -> Self {
        self.threads = n;
        self
    }

    #[must_use]
    pub fn ctx_size(mut self, n: u32) -> Self {
        self.ctx_size = n;
        self
    }

    #[must_use]
    pub fn startup_timeout(mut self, d: Duration) -> Self {
        self.startup_timeout = d;
        self
    }

    /// Build and start the server.
    ///
    /// # Errors
    ///
    /// Returns an error if the binary/model is missing or if the server
    /// doesn't start within the timeout.
    pub fn build(self) -> Result<LlamaServer, LlamaServerError> {
        LlamaServer::start_with(self)
    }
}

/// Errors from the llama-server harness.
#[derive(Debug, derive_more::Display, derive_more::Error)]
pub enum LlamaServerError {
    #[display("llama-server binary not found at {_0}. Run: mise run llama:server:build")]
    #[error(ignore)]
    BinaryNotFound(String),

    #[display("model file not found at {_0}. Run: mise run llama:test-model:download")]
    #[error(ignore)]
    ModelNotFound(String),

    #[display("failed to spawn llama-server: {_0}")]
    SpawnFailed(std::io::Error),

    #[display("llama-server did not become healthy within {_0} seconds")]
    #[error(ignore)]
    HealthTimeout(u64),
}

fn kill_existing(port: u16) {
    let _ = Command::new("fuser")
        .args(["-k", &format!("{port}/tcp")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(Duration::from_millis(500));
}

fn wait_for_health(port: u16, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(stream) = TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_secs(1),
        ) {
            let request = format!(
                "GET /health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
            );
            let _ = (&stream).write_all(request.as_bytes());
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let reader = BufReader::new(&stream);
            if let Some(Ok(line)) = reader.lines().next() {
                if line.contains("200") {
                    return true;
                }
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    false
}
