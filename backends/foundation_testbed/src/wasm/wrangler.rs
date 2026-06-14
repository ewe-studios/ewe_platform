//! Wrangler dev subprocess + HTTP test runner.
//!
//! WHY: Cloudflare Workers runtime is different from browser/Deno — tests in
//!      the actual Workers runtime catch runtime-specific issues.
//! WHAT: Starts `wrangler dev` on a random port, sends HTTP request, reads response.
//! HOW: Spawns wrangler, polls until the HTTP server is ready via a raw TCP GET,
//!      then kills the wrangler process.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

use tracing::{debug, info};

use crate::wasm::error::{Result, ToTrace, WasmTestbedError};

/// Output from a wrangler test run.
pub struct WranglerOutput {
    /// HTTP response body from the worker.
    pub response_body: String,
    /// HTTP status code from the worker.
    pub status_code: u16,
    /// Wrangler's stderr output (useful for debugging).
    pub wrangler_stderr: String,
}

/// Simple HTTP GET via raw `TcpStream`.
///
/// WHY: We only need a single GET to poll if wrangler dev is ready.
/// WHAT: Writes raw HTTP bytes, reads the response.
/// HOW: Opens a `TcpStream`, writes `GET / HTTP/1.1` + host headers,
///      reads until EOF or timeout.
fn http_get(host: &str, port: u16) -> Result<(u16, String)> {
    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect(&addr)
        .map_err(|e| WasmTestbedError::WranglerHttpFailed(format!("connect {addr}: {e}")).trace())?;

    stream.set_read_timeout(Some(Duration::from_secs(1)))
        .map_err(|e| WasmTestbedError::WranglerHttpFailed(format!("set timeout: {e}")).trace())?;

    let request = format!("GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes())
        .map_err(|e| WasmTestbedError::WranglerHttpFailed(format!("write: {e}")).trace())?;

    let mut response = String::new();
    stream.read_to_string(&mut response)
        .map_err(|e| WasmTestbedError::WranglerHttpFailed(format!("read: {e}")).trace())?;

    // Parse status code from first line: "HTTP/1.1 200 OK"
    let status_code = response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);

    // Split headers from body
    let body = response
        .split_once("\r\n\r\n")
        .map_or(response.as_str(), |(_, body)| body)
        .to_string();

    Ok((status_code, body))
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
pub fn run(integration_dir: &Path, _config_path: Option<&Path>) -> Result<WranglerOutput> {
    which::which("wrangler").map_err(|_| WasmTestbedError::WranglerNotFound.trace())?;

    let port = portpicker::pick_unused_port()
        .ok_or_else(|| WasmTestbedError::NoPortAvailable.trace())?;

    info!("Starting wrangler dev on port {port}...");

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
        .map_err(|e| WasmTestbedError::WranglerStartFailed(e).trace())?;

    let mut stderr_bytes = Vec::new();

    // Poll until wrangler is ready (up to 10 seconds)
    for attempt in 0..100 {
        thread::sleep(Duration::from_millis(100));

        match http_get("127.0.0.1", port) {
            Ok((status_code, response_body)) => {
                info!("Wrangler ready after {}ms", attempt * 100);

                // Kill wrangler
                let _ = child.kill();
                let _ = child.wait();

                // Collect stderr
                if let Some(mut stderr) = child.stderr.take() {
                    let _ = stderr.read_to_end(&mut stderr_bytes);
                }
                let stderr = String::from_utf8_lossy(&stderr_bytes).to_string();

                return Ok(WranglerOutput {
                    response_body,
                    status_code,
                    wrangler_stderr: stderr,
                });
            }
            Err(_) => {
                debug!("Wrangler not ready yet (attempt {})", attempt + 1);
            }
        }
    }

    // Timeout — kill wrangler and report
    let _ = child.kill();
    let _ = child.wait();

    let mut stderr = String::new();
    if let Some(mut s) = child.stderr.take() {
        let _ = s.read_to_string(&mut stderr);
    }

    Err(WasmTestbedError::WranglerTimeout(stderr).trace())
}
