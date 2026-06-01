//! Wrangler dev subprocess + HTTP test runner.
//!
//! WHY: Cloudflare Workers runtime is different from browser/Deno — tests in
//!      the actual Workers runtime catch runtime-specific issues.
//! WHAT: Starts `wrangler dev` on a random port, sends HTTP request, reads response.
//! HOW: Spawns wrangler, polls until the HTTP server is ready, sends GET request,
//!      then kills the wrangler process.

use std::path::Path;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

use tracing::{debug, info};

/// Output from a wrangler test run.
pub struct WranglerOutput {
    /// HTTP response body from the worker.
    pub response_body: String,
    /// HTTP status code from the worker.
    pub status_code: u16,
    /// Wrangler's stderr output (useful for debugging).
    pub wrangler_stderr: String,
}

/// Run a wrangler dev test.
///
/// Starts wrangler dev on a random port, waits for it to be ready,
/// sends an HTTP GET request to the root path, and returns the response.
///
/// # Errors
///
/// Returns an error if:
/// - wrangler is not on PATH
/// - wrangler dev does not become ready within 10 seconds
/// - HTTP request fails after wrangler is ready
pub fn run(integration_dir: &Path, _config_path: Option<&Path>) -> anyhow::Result<WranglerOutput> {
    // Verify wrangler is available
    which::which("wrangler").map_err(|_| {
        anyhow::anyhow!(
            "wrangler not found on PATH.\n\
            Install: npm install -g wrangler"
        )
    })?;

    // Pick a random port
    let port = portpicker::pick_unused_port()
        .ok_or_else(|| anyhow::anyhow!("No available port found"))?;

    info!("Starting wrangler dev on port {port}...");

    // Start wrangler
    let mut child = Command::new("wrangler")
        .arg("dev")
        .arg("--port")
        .arg(port.to_string())
        .arg("--ip")
        .arg("127.0.0.1")
        .current_dir(integration_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to start wrangler: {e}"))?;

    // Poll until wrangler is ready (up to 10 seconds)
    let base_url = format!("http://127.0.0.1:{port}");
    for attempt in 0..100 {
        thread::sleep(Duration::from_millis(100));

        match reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(1))
            .build()
            .ok()
            .and_then(|client| client.get(&base_url).send().ok())
        {
            Some(response) => {
                let status_code = response.status().as_u16();
                let response_body = response.text().unwrap_or_default();

                // Kill wrangler
                kill_process(&mut child);

                let stderr = read_stderr(&mut child);

                return Ok(WranglerOutput {
                    response_body,
                    status_code,
                    wrangler_stderr: stderr,
                });
            }
            None => {
                debug!("Wrangler not ready yet (attempt {})", attempt + 1);
            }
        }
    }

    // Timeout — kill wrangler and report
    kill_process(&mut child);
    let stderr = read_stderr(&mut child);

    anyhow::bail!(
        "wrangler dev did not become ready within 10 seconds\n\
        wrangler stderr:\n{stderr}"
    )
}

/// Kill a process and its children.
fn kill_process(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Read remaining stderr from a child process.
fn read_stderr(child: &mut Child) -> String {
    use std::io::Read;

    if let Some(mut stderr) = child.stderr.take() {
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        return buf;
    }
    String::new()
}
