//! Deno test runner — invokes `deno run --allow-read --allow-net`.
//!
//! WHY: Deno provides a fast, native wasm testing environment.
//! WHAT: Runs a JS entry point file via Deno with filesystem and network permissions.
//! HOW: Spawns deno as a subprocess, captures stdout/stderr, returns exit code.

use std::path::Path;
use std::process::Command;

use tracing::{debug, info};

/// Output from a Deno test run.
pub struct DenoOutput {
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Exit code from the deno process.
    pub exit_code: i32,
}

/// Run a JS file via Deno with appropriate permissions.
///
/// The entry file is resolved relative to `integration_dir`.
///
/// # Errors
///
/// Returns an error if:
/// - deno is not on PATH
/// - deno exits with a non-zero code (error includes captured output)
pub fn run(integration_dir: &Path, entry_file: &str) -> anyhow::Result<DenoOutput> {
    // Verify deno is available
    which::which("deno").map_err(|_| {
        anyhow::anyhow!(
            "deno not found on PATH.\n\
            Install from https://deno.land"
        )
    })?;

    info!("Running deno test: {entry_file}");

    let entry_path = integration_dir.join(entry_file);
    if !entry_path.exists() {
        anyhow::bail!(
            "Entry file not found: {}\n\
            Hint: run wasm-testbed init deno ./crate first.",
            entry_path.display()
        );
    }

    let mut cmd = Command::new("deno");
    cmd.arg("run")
        .arg("--allow-read")
        .arg("--allow-net")
        .arg("--no-check")
        .arg(&entry_path)
        .current_dir(integration_dir);

    debug!("Executing: {:?}", cmd);

    let output = cmd.output().map_err(|e| {
        anyhow::anyhow!("Failed to execute deno: {e}")
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    if !stdout.is_empty() {
        debug!("deno stdout:\n{stdout}");
    }
    if !stderr.is_empty() {
        debug!("deno stderr:\n{stderr}");
    }

    if exit_code != 0 {
        anyhow::bail!(
            "deno test failed (exit code {exit_code})\n\
            stdout:\n{stdout}\n\
            stderr:\n{stderr}"
        );
    }

    Ok(DenoOutput {
        stdout,
        stderr,
        exit_code,
    })
}
