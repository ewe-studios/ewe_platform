//! # Browser process supervision (spec-43 phase-1 §4)
//!
//! WHY: A test owns a real browser process. Launch must be hermetic (a throwaway
//! profile), the CDP endpoint must be discovered reliably, and teardown must kill
//! the process + remove the profile **no matter how the test ends** — so this is
//! an RAII guard whose `Drop` reaps everything (never panicking).
//!
//! WHAT: [`BrowserProcess`] — spawn, discover `ws_url`, kill on drop.
//!
//! HOW: Launch Chromium with `--remote-debugging-port=0` + a temp `--user-data-dir`;
//! Chromium writes `DevToolsActivePort` (`<port>\n<ws-path>`) into that dir when
//! ready. Poll for it (deadline), build `ws://127.0.0.1:<port><path>`. On `Drop`:
//! `kill` + `wait` (reap the child), then remove the temp profile.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::cdp::launch::LaunchConfig;
use crate::error::{BrowserError, Result};

/// A launched browser process + its discovered CDP endpoint.
pub struct BrowserProcess {
    child: Child,
    profile_dir: PathBuf,
    ws_url: String,
}

impl BrowserProcess {
    /// Launch the browser and discover its CDP `webSocketDebuggerUrl`.
    ///
    /// # Errors
    /// [`BrowserError::BrowserNotInstalled`] if the binary is missing,
    /// [`BrowserError::Launch`] on spawn/discovery failure.
    pub fn launch(config: &LaunchConfig) -> Result<Self> {
        let binary = resolve_binary(config)?;
        let profile_dir = unique_profile_dir();
        std::fs::create_dir_all(&profile_dir)?;

        let mut args = config.base_args();
        args.push(format!("--user-data-dir={}", profile_dir.display()));
        args.push("--remote-debugging-port=0".to_string());
        args.push("about:blank".to_string());

        tracing::debug!(?binary, ?args, "launching browser");
        let child = Command::new(&binary)
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| BrowserError::Launch(format!("spawn {binary}: {e}")))?;

        let ws_url = match discover_ws_url(&profile_dir, Duration::from_secs(20)) {
            Ok(url) => url,
            Err(e) => {
                // Don't leak the process if discovery failed.
                let mut dead = child;
                let _ = dead.kill();
                let _ = dead.wait();
                let _ = std::fs::remove_dir_all(&profile_dir);
                return Err(e);
            }
        };

        Ok(Self { child, profile_dir, ws_url })
    }

    /// The browser-level CDP WebSocket URL to connect the engine to.
    #[must_use]
    pub fn ws_url(&self) -> &str {
        &self.ws_url
    }
}

impl Drop for BrowserProcess {
    fn drop(&mut self) {
        if let Err(e) = self.child.kill() {
            tracing::warn!("failed to kill browser process: {e}");
        }
        let _ = self.child.wait();
        if let Err(e) = std::fs::remove_dir_all(&self.profile_dir) {
            tracing::warn!("failed to remove browser profile dir: {e}");
        }
    }
}

/// Resolve the browser binary: `*_BIN` env var, then candidates on `PATH`.
fn resolve_binary(config: &LaunchConfig) -> Result<String> {
    let browser = config.browser;
    if let Ok(path) = std::env::var(browser.bin_env()) {
        if !path.is_empty() {
            return Ok(path);
        }
    }
    for candidate in browser.binaries() {
        if which(candidate).is_some() {
            return Ok((*candidate).to_string());
        }
    }
    Err(BrowserError::BrowserNotInstalled {
        binary: browser.binaries()[0].to_string(),
        mise_task: browser.mise_task().to_string(),
    })
}

/// Minimal `which`: scan `PATH` for an executable file.
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// A unique throwaway profile directory under the system temp dir.
fn unique_profile_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("primal-browser-{}-{nanos}", std::process::id()))
}

/// Poll the `DevToolsActivePort` file Chromium writes into the profile dir;
/// build `ws://127.0.0.1:<port><path>` from its two lines.
fn discover_ws_url(profile_dir: &std::path::Path, deadline: Duration) -> Result<String> {
    let port_file = profile_dir.join("DevToolsActivePort");
    let start = Instant::now();
    loop {
        if let Ok(mut f) = std::fs::File::open(&port_file) {
            let mut contents = String::new();
            if f.read_to_string(&mut contents).is_ok() {
                let mut lines = contents.lines();
                if let (Some(port), Some(path)) = (lines.next(), lines.next()) {
                    if !port.trim().is_empty() && path.starts_with('/') {
                        return Ok(format!("ws://127.0.0.1:{}{}", port.trim(), path.trim()));
                    }
                }
            }
        }
        if start.elapsed() >= deadline {
            return Err(BrowserError::Launch(format!(
                "browser did not expose a CDP endpoint within {}s (no DevToolsActivePort)",
                deadline.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
