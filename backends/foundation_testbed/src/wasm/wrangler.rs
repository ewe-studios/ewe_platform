//! Wrangler dev subprocess + HTTP test runner.
//!
//! WHY: Cloudflare Workers runtime is different from browser/Deno — tests in
//!      the actual Workers runtime catch runtime-specific issues, and
//!      `wrangler dev --local` serves real local D1, R2 and KV bindings.
//! WHAT: Starts `wrangler dev --local` on a free port, sends one HTTP GET that
//!      runs the worker (and the wasm tests in it), and reads the response.
//! HOW: Spawns wrangler (or the pinned `npx wrangler@<version>` when no
//!      `wrangler` binary is on PATH), polls until the HTTP server accepts a
//!      connection, then drains wrangler's stderr on a reader thread and kills
//!      the process.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use foundation_core::valtron::Ctx;
use tracing::{debug, info};

use crate::wasm::error::{Result, ToTrace, WasmTestbedError};

/// The wrangler version the harness uses when it falls back to `npx`.
///
/// The local explorer REST API and the local bindings are beta, so the version
/// is pinned. The environment variable `WRANGLER_VERSION` (set by the project
/// `mise.toml`) overrides the default.
pub(crate) const DEFAULT_WRANGLER_VERSION: &str = "4.156.2";

/// Timeouts and version for one `wrangler dev` run.
pub(crate) struct WranglerConfig {
    /// How long to wait for `wrangler dev` to accept a connection.
    /// Default: 120 s (`npx` downloads the pinned wrangler on first use, and
    /// wrangler unpacks workerd before it serves).
    pub startup_timeout: Duration,
    /// Read timeout for one `/health` readiness probe.
    /// Default: 2 s. A probe that times out is retried until `startup_timeout`.
    pub startup_probe_timeout: Duration,
    /// Read timeout for the tail of a response that sent no `Content-Length`
    /// (a chunked error page, usually). Default: 5 s.
    pub headerless_body_timeout: Duration,
    /// Read timeout for the HTTP request that runs the worker tests.
    /// Default: 300 s (a wasm suite over real D1, R2 and KV storage).
    pub request_timeout: Duration,
    /// wrangler version for the `npx` fallback.
    pub version: String,
}

impl Default for WranglerConfig {
    fn default() -> Self {
        Self {
            startup_timeout: Duration::from_secs(120),
            startup_probe_timeout: Duration::from_secs(2),
            headerless_body_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(300),
            version: std::env::var("WRANGLER_VERSION")
                .unwrap_or_else(|_| DEFAULT_WRANGLER_VERSION.to_string()),
        }
    }
}

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
/// WHY: We only need plain GETs — a readiness probe and the test run.
/// WHAT: Writes raw HTTP bytes, reads the status line, headers and the body.
/// HOW: Reads `Content-Length` bytes when the server sends the header, so a
///      kept-alive connection cannot block the read. A response without the
///      header is read until EOF or the read timeout.
fn http_get(
    host: &str,
    port: u16,
    path: &str,
    read_timeout: Duration,
    tail_timeout: Duration,
) -> Result<(u16, String)> {
    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect(&addr).map_err(|e| {
        WasmTestbedError::WranglerHttpFailed(format!("connect {addr}: {e}")).trace()
    })?;

    stream
        .set_read_timeout(Some(read_timeout))
        .map_err(|e| WasmTestbedError::WranglerHttpFailed(format!("set timeout: {e}")).trace())?;

    let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|e| WasmTestbedError::WranglerHttpFailed(format!("write: {e}")).trace())?;

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader
        .read_line(&mut status_line)
        .map_err(|e| WasmTestbedError::WranglerHttpFailed(format!("read status: {e}")).trace())?;
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);

    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).map_err(|e| {
            WasmTestbedError::WranglerHttpFailed(format!("read header: {e}")).trace()
        })?;
        if read == 0 {
            break;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().ok();
        }
    }

    let body = if let Some(length) = content_length {
        let mut bytes = vec![0_u8; length];
        reader
            .read_exact(&mut bytes)
            .map_err(|e| WasmTestbedError::WranglerHttpFailed(format!("read body: {e}")).trace())?;
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        // No length (chunked): read what arrives, with a short tail timeout. A
        // timed-out tail still leaves the bytes received so far.
        let _ = reader.get_ref().set_read_timeout(Some(tail_timeout));
        let mut bytes = Vec::new();
        match reader.read_to_end(&mut bytes) {
            Ok(_) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => {
                return Err(
                    WasmTestbedError::WranglerHttpFailed(format!("read body: {e}")).trace(),
                );
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    };

    Ok((status_code, body))
}

/// The command that starts wrangler: `WRANGLER_BIN` if set, else the pinned
/// version through `npx` when `WRANGLER_VERSION` is set, else a `wrangler`
/// binary on PATH, else the default pinned version through `npx`.
fn wrangler_command(config: &WranglerConfig) -> Result<(String, Vec<String>)> {
    if let Ok(bin) = std::env::var("WRANGLER_BIN") {
        return Ok((bin, Vec::new()));
    }
    let version_pinned = std::env::var("WRANGLER_VERSION").is_ok();
    if !version_pinned && which::which("wrangler").is_ok() {
        return Ok(("wrangler".to_string(), Vec::new()));
    }
    which::which("npx").map_err(|_| WasmTestbedError::WranglerNotFound.trace())?;
    Ok((
        "npx".to_string(),
        vec!["--yes".to_string(), format!("wrangler@{}", config.version)],
    ))
}

/// Spawn wrangler with its stderr written to a log file.
///
/// A pipe would block the runner: `npx` starts wrangler, which starts workerd,
/// and the descendants keep the pipe's write end open after `Child::kill`. The
/// file needs no reader thread and can be read after the process group dies.
fn start_wrangler(
    integration_dir: &Path,
    port: u16,
    config: &WranglerConfig,
) -> Result<(Child, std::path::PathBuf)> {
    let (program, prefix_args) = wrangler_command(config)?;
    info!("Starting {program} dev --local on port {port}...");
    let log_path = integration_dir.join(".wrangler-testbed.log");
    let log = std::fs::File::create(&log_path).map_err(|e| WasmTestbedError::Io(e).trace())?;
    let log_err = log
        .try_clone()
        .map_err(|e| WasmTestbedError::Io(e).trace())?;

    let mut command = Command::new(&program);
    command
        .args(&prefix_args)
        .arg("dev")
        .arg("--local")
        .arg("--port")
        .arg(port.to_string())
        .arg("--ip")
        .arg("127.0.0.1")
        .current_dir(integration_dir)
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));
    // A new process group lets the runner stop workerd too.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let child = command
        .spawn()
        .map_err(|e| WasmTestbedError::WranglerStartFailed(e).trace())?;
    Ok((child, log_path))
}

/// Stop wrangler and every process it started (its process group on Unix).
fn stop_wrangler(child: &mut Child) {
    #[cfg(unix)]
    {
        // A negative PID signals the whole process group.
        let _ = unsafe { libc::kill(-child.id().cast_signed(), libc::SIGKILL) };
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Read the log file wrangler wrote, ignoring a missing or non-UTF-8 file.
fn read_wrangler_log(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// Run a wrangler dev test with the default [`WranglerConfig`].
///
/// # Errors
///
/// Returns an error if wrangler (or `npx`) is not on PATH, wrangler does not
/// accept a connection within the startup timeout, or the HTTP request fails.
pub fn run(
    integration_dir: &Path,
    _config_path: Option<&Path>,
    ctx: &Ctx,
) -> Result<WranglerOutput> {
    run_with_config(integration_dir, &WranglerConfig::default(), ctx)
}

/// Run a wrangler dev test with an explicit [`WranglerConfig`].
///
/// # Errors
///
/// Returns an error if wrangler (or `npx`) is not on PATH, wrangler does not
/// accept a connection within the startup timeout, or the HTTP request fails.
pub(crate) fn run_with_config(
    integration_dir: &Path,
    config: &WranglerConfig,
    ctx: &Ctx,
) -> Result<WranglerOutput> {
    let clock = ctx.clock();
    let port =
        portpicker::pick_unused_port().ok_or_else(|| WasmTestbedError::NoPortAvailable.trace())?;

    let (mut child, log_path) = start_wrangler(integration_dir, port, config)?;

    // Poll the worker's `/health` route until it answers. A raw TCP connect is
    // not enough: workerd accepts connections before the module finishes
    // loading, and the test request would wait for the build.
    let deadline = clock.now() + config.startup_timeout;
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            let log = read_wrangler_log(&log_path);
            return Err(
                WasmTestbedError::WranglerStartFailed(std::io::Error::other(format!(
                    "wrangler exited with {status} before serving: {log}"
                )))
                .trace(),
            );
        }
        match http_get(
            "127.0.0.1",
            port,
            "/health",
            config.startup_probe_timeout,
            config.headerless_body_timeout,
        ) {
            Ok((200, _)) => break,
            Ok((status, _)) if clock.now() >= deadline => {
                stop_wrangler(&mut child);
                return Err(WasmTestbedError::WranglerHttpFailed(format!(
                    "the /health route answered {status}: {}",
                    read_wrangler_log(&log_path)
                ))
                .trace());
            }
            _ if clock.now() < deadline => {
                debug!("wrangler not ready yet (port {port})");
                thread::sleep(Duration::from_millis(100));
            }
            _ => {
                stop_wrangler(&mut child);
                return Err(
                    WasmTestbedError::WranglerTimeout(read_wrangler_log(&log_path)).trace(),
                );
            }
        }
    }

    info!("wrangler ready on port {port}, running the worker tests...");
    let response = http_get(
        "127.0.0.1",
        port,
        "/",
        config.request_timeout,
        config.headerless_body_timeout,
    );

    stop_wrangler(&mut child);

    let (status_code, response_body) = response?;
    Ok(WranglerOutput {
        response_body,
        status_code,
        wrangler_stderr: read_wrangler_log(&log_path),
    })
}
