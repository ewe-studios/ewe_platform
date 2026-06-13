//! Browser test runner — pure-Rust, no node/Playwright (spec-43).
//!
//! WHY: Browser tests need a real browser with DOM + WebAssembly. We drive a
//! system Chromium directly over the Chrome DevTools Protocol via the
//! `foundation_browser` driver — no Node.js, no `npm install`, no Playwright.
//! WHAT: [`run`] launches Chromium, navigates to the test page, and polls the
//! `#output` element for the test-result sentinel.
//! HOW: `foundation_browser::BrowserDriver` (CDP over our own WebSocket client);
//! the page writes results into `#output`, which we read with `Page::eval`.

use std::time::{Duration, Instant};

use foundation_browser::{BrowserDriver, LaunchConfig};
use tracing::{debug, info};

use crate::cli::Browser;
use crate::error::{Result, ToTrace, WasmTestbedError};

/// Output from a browser test run.
#[derive(Debug)]
pub struct BrowserOutput {
    /// The first line of `#output` (e.g. `test result: ok. …`).
    pub test_result: String,
    /// Captured console logs (best-effort; currently unused by callers).
    pub console_logs: Vec<String>,
    /// Process-style exit code: `0` once results were captured (callers derive
    /// pass/fail from `test_result`).
    pub exit_code: i32,
}

/// How long to wait for the page to produce a result before giving up.
const RESULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Run the wasm test page at `url` in a real Chromium and capture its result.
///
/// # Errors
/// [`WasmTestbedError::BrowserDriver`] if Chromium can't launch/navigate or the
/// page doesn't produce a result within [`RESULT_TIMEOUT`]; for an unsupported
/// browser (only Chromium ships in phase 1).
pub fn run(url: &str, browser: &Browser, headless: bool) -> Result<BrowserOutput> {
    // The pure-Rust driver speaks CDP — Chromium only (Firefox/WebKit are the
    // spec-43 phase-2 WebDriver-BiDi follow-up).
    if !matches!(browser, Browser::Chrome) {
        return Err(WasmTestbedError::BrowserDriver(format!(
            "{browser:?} is not supported by the pure-Rust CDP driver yet — use Chrome"
        ))
        .trace());
    }

    info!("Running browser test: {url} (headless={headless})");

    let driver = BrowserDriver::launch(LaunchConfig::chromium().headless(headless))
        .map_err(|e| WasmTestbedError::BrowserDriver(format!("launch: {e}")).trace())?;
    let page = driver
        .new_page()
        .map_err(|e| WasmTestbedError::BrowserDriver(format!("attach page: {e}")).trace())?;
    page.goto(url)
        .map_err(|e| WasmTestbedError::BrowserDriver(format!("navigate {url}: {e}")).trace())?;

    // Poll #output for the result sentinel the test harness writes.
    let deadline = Instant::now() + RESULT_TIMEOUT;
    let output = loop {
        let text = page
            .eval("(document.querySelector('#output') || {}).textContent || ''")
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        if text.contains("test result:") || text.contains("Tests complete") {
            break text;
        }
        if Instant::now() >= deadline {
            return Err(WasmTestbedError::BrowserDriver(format!(
                "test did not produce results within {}s",
                RESULT_TIMEOUT.as_secs()
            ))
            .trace());
        }
        std::thread::sleep(Duration::from_millis(500));
    };

    let test_result = output.lines().next().unwrap_or("").trim().to_string();
    debug!("Browser test result: {test_result}");

    Ok(BrowserOutput { test_result, console_logs: Vec::new(), exit_code: 0 })
}
