//! Self-contained llama-server test harness.
//!
//! Starts llama-server as a child process, waits for the health endpoint,
//! and kills it on drop. Tests no longer depend on `mise run llama:server:start`.
//!
//! The harness finds the project root via `CARGO_MANIFEST_DIR`, validates the
//! server binary and model file exist, and exposes the connection details
//! through `LlamaServerGuard`.

#![allow(dead_code)]

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

/// Connection details for a running llama-server.
pub struct LlamaServerGuard {
    child: Option<Child>,
    pub port: u16,
    pub api_key: String,
    pub model_name: String,
}

impl LlamaServerGuard {
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for LlamaServerGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            eprintln!("[llama-harness] stopping llama-server (pid={})", child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Configuration for the test server.
pub struct LlamaServerConfig {
    pub port: u16,
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
                .and_then(|p| p.parse().ok())
                .unwrap_or(8999),
            api_key: "test-api-key-123".into(),
            model_name: std::env::var("LLAMA_TEST_MODEL_NAME")
                .unwrap_or_else(|_| "qwen2.5-0.5b-instruct".into()),
            model_file: std::env::var("LLAMA_TEST_MODEL_FILE").map_or_else(|_| root.join("artefacts/models/qwen2.5-0.5b-instruct-q4_k_m.gguf"), PathBuf::from),
            server_bin: root.join("support/bin/llama-server"),
            threads: 4,
            ctx_size: 2048,
            batch_size: 512,
            ubatch_size: 256,
            startup_timeout: Duration::from_secs(60),
        }
    }
}

/// Kill any process already listening on the port.
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

/// Start a llama-server with the default configuration.
///
/// Panics if the server binary or model file is missing, or if the server
/// doesn't become healthy within the timeout.
///
/// Returns a `LlamaServerGuard` that kills the server on drop.
#[must_use]
pub fn start_llama_server() -> LlamaServerGuard {
    start_llama_server_with(LlamaServerConfig::default())
}

/// Start a llama-server with the given configuration.
#[must_use]
pub fn start_llama_server_with(config: LlamaServerConfig) -> LlamaServerGuard {
    assert!(
        config.server_bin.exists(),
        "llama-server not found at {}. Run: mise run llama:server:build",
        config.server_bin.display()
    );
    assert!(
        config.model_file.exists(),
        "test model not found at {}. Run: mise run llama:test-model:download",
        config.model_file.display()
    );

    kill_existing(config.port);

    eprintln!(
        "[llama-harness] starting llama-server on port {} with model {}",
        config.port,
        config.model_file.display()
    );

    let child = Command::new(&config.server_bin)
        .args([
            "--model",
            config.model_file.to_str().unwrap(),
            "--host",
            "127.0.0.1",
            "--port",
            &config.port.to_string(),
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
        .unwrap_or_else(|e| panic!("failed to spawn llama-server: {e}"));

    let pid = child.id();
    eprintln!("[llama-harness] llama-server spawned (pid={pid}), waiting for health check");

    let healthy = wait_for_health(config.port, config.startup_timeout);

    if !healthy {
        // Kill via Drop before panicking.
        let guard = LlamaServerGuard {
            child: Some(child),
            port: config.port,
            api_key: config.api_key,
            model_name: config.model_name,
        };
        drop(guard);
        panic!(
            "llama-server did not become healthy within {} seconds",
            config.startup_timeout.as_secs()
        );
    }

    eprintln!(
        "[llama-harness] llama-server is ready (pid={pid}, port={})",
        config.port
    );

    LlamaServerGuard {
        child: Some(child),
        port: config.port,
        api_key: config.api_key,
        model_name: config.model_name,
    }
}
