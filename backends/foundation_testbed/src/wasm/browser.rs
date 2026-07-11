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

use crate::wasm::cli::Browser;
use crate::wasm::error::{Result, ToTrace, WasmTestbedError};

#[derive(Debug)]
pub struct BrowserOutput {
    pub test_result: String,
    pub console_logs: Vec<String>,
    pub exit_code: i32,
}

const RESULT_TIMEOUT: Duration = Duration::from_secs(30);

pub fn run(url: &str, browser: &Browser, headless: bool) -> Result<BrowserOutput> {
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

    // Poll the page after navigation. We don't rely on Page.loadEventFired
    // because type="module" scripts may execute AFTER that event.
    // Instead: just navigate and immediately start polling.
    //
    // page.goto() internally subscribes to Page.loadEventFired, fires
    // Page.navigate, and blocks until the event. After it returns the
    // page is loaded (static HTML parsed, module scripts fetched). The
    // top-level await in the module then executes — typically within a few
    // hundred ms for our lightweight test.
    page.goto(url)
        .map_err(|e| WasmTestbedError::BrowserDriver(format!("navigate {url}: {e}")).trace())?;

    // Give module scripts time to resolve top-level await.
    // Page.loadEventFired (waited in `goto`) fires before type="module"
    // scripts execute their async body. A brief wall-clock sleep lets the
    // browser fetch and execute the module graph.
    std::thread::sleep(Duration::from_secs(3));

    let deadline = Instant::now() + RESULT_TIMEOUT;
    let output = loop {
        let text = page
            .eval("(document.querySelector('#output') || {}).textContent || ''")
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        if text.contains("test result:") {
            break text;
        }
        if Instant::now() >= deadline {
            let diagnostic = page
                .eval("JSON.stringify({title:document.title||'',hasOutput:!!document.querySelector('#output'),body:(document.body?.textContent||'').substring(0,500)})")
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            return Err(WasmTestbedError::BrowserDriver(format!(
                "test did not produce results within {}s\ndiagnostic: {diagnostic}",
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
