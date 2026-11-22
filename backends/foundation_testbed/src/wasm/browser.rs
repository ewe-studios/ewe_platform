//! Browser test runner — pure-Rust, no node/Playwright (spec-43).
//!
//! WHY: Browser tests need a real browser with DOM + WebAssembly. We drive a
//! system Chromium directly over the Chrome `DevTools` Protocol via the
//! `foundation_browser` driver — no Node.js, no `npm install`, no Playwright.
//! WHAT: [`run`] launches Chromium, navigates to the test page, and polls the
//! `#output` element for the test-result sentinel.
//! HOW: `foundation_browser::BrowserDriver` (CDP over our own WebSocket client);
//! the page writes results into `#output`, which we read with `Page::eval`.

use std::time::Duration;

use foundation_browser::pause::pause;
use foundation_browser::{BrowserDriver, LaunchConfig, WaitConfig};
use foundation_context::Ctx;
use tracing::{debug, info};

use crate::wasm::cli::Browser;
use crate::wasm::error::{Result, ToTrace, WasmTestbedError};

#[derive(Debug)]
pub struct BrowserOutput {
    /// Every line the page printed, in order: the case lines, then the summary.
    ///
    /// A failing run must name its case and its first mismatched value, so the
    /// runner keeps this text whole (spec 88 rule R3).
    pub output: String,
    /// The last `test result:` line: the verdict.
    pub test_result: String,
    pub console_logs: Vec<String>,
    pub exit_code: i32,
}

/// Limits of one browser run. Every field has a builder method and a
/// documented default (owner rule 8).
#[derive(Clone, Debug)]
pub struct BrowserRunLimits {
    result_timeout: Duration,
    module_settle: Duration,
    result_poll: Duration,
}

impl Default for BrowserRunLimits {
    fn default() -> Self {
        Self {
            // The owned harness reports after a wasm module loads and runs its
            // cases. The sample module finished in about a second; 30 s covers
            // a cold machine and still fails a hung page in bounded time.
            result_timeout: Duration::from_secs(30),
            // A module script runs its top-level await after Page.loadEventFired.
            // Three seconds is the margin the runner used before this value
            // became configuration.
            module_settle: Duration::from_secs(3),
            // Two reads per second see the summary soon without busy-spinning.
            result_poll: Duration::from_millis(500),
        }
    }
}

impl BrowserRunLimits {
    /// Set how long the run waits for the `test result:` summary.
    #[must_use]
    pub fn with_result_timeout(mut self, timeout: Duration) -> Self {
        self.result_timeout = timeout;
        self
    }

    /// Set how long the run waits after load for module scripts to settle.
    #[must_use]
    pub fn with_module_settle(mut self, settle: Duration) -> Self {
        self.module_settle = settle;
        self
    }

    /// Set the pause between two reads of the `#output` element.
    #[must_use]
    pub fn with_result_poll(mut self, poll: Duration) -> Self {
        self.result_poll = poll;
        self
    }
}

/// Check that the testbed can drive `browser`.
///
/// Only Chrome is supported. The Firefox `WebDriver` `BiDi` backend exists in
/// `foundation_browser`, but no wasm suite runs on it yet (spec 88 section 11).
///
/// # Errors
/// [`WasmTestbedError::BrowserDriver`] for every other browser.
pub(crate) fn ensure_supported(browser: &Browser) -> Result<()> {
    if matches!(browser, Browser::Chrome) {
        return Ok(());
    }
    Err(WasmTestbedError::BrowserDriver(format!(
        "{browser:?} is not supported: the testbed drives Chrome over the Chrome DevTools \
         Protocol (CDP). The Firefox WebDriver BiDi backend has no wasm suite yet \
         (spec 88 section 11)."
    ))
    .trace())
}

/// Run a browser test and collect its output.
///
/// # Errors
///
/// Returns an error if the browser is not supported, the page cannot be
/// opened, or the result does not arrive before the timeout.
pub fn run(
    url: &str,
    browser: &Browser,
    headless: bool,
    ctx: &Ctx,
    limits: &BrowserRunLimits,
) -> Result<BrowserOutput> {
    ensure_supported(browser)?;

    info!("Running browser test: {url} (headless={headless})");

    let clock = ctx.clock();
    let driver = BrowserDriver::launch_borrowed(LaunchConfig::chromium().headless(headless), ctx)
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
    // scripts execute their async body. A short pause on the injected clock
    // lets the browser fetch and execute the module graph. A simulated clock
    // steps this pause, so a test does not wait in real time.
    let wait = WaitConfig::default();
    pause(ctx, limits.module_settle, &wait);

    let deadline = clock.now() + limits.result_timeout;
    let output = loop {
        let text = page
            .eval("(document.querySelector('#output') || {}).textContent || ''")
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        if text.contains("test result:") {
            break text;
        }
        if clock.now() >= deadline {
            // A trap inside a host callback ends the page's module before the
            // summary. The runner exposes the case in flight through
            // `__FWT_DIAGNOSTICS__`, so the timeout names it (issue #587).
            let diagnostic = page
                .eval("JSON.stringify({title:document.title||'',hasOutput:!!document.querySelector('#output'),body:(document.body?.textContent||'').substring(0,500),diagnostics:(globalThis.__FWT_DIAGNOSTICS__ && globalThis.__FWT_DIAGNOSTICS__()) || null})")
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            return Err(WasmTestbedError::BrowserDriver(format!(
                "test did not produce results within {}s\ndiagnostic: {diagnostic}",
                limits.result_timeout.as_secs()
            ))
            .trace());
        }
        pause(ctx, limits.result_poll, &wait);
    };

    debug!("browser page output:\n{output}");
    // #output holds every printed line: each case line, then the summary. The
    // summary is the verdict, so take the last line that carries it. The whole
    // text stays in `output`: a failing case names itself there (issue #588).
    let test_result = output
        .lines()
        .rfind(|line| line.contains("test result:"))
        .unwrap_or("")
        .trim()
        .to_string();
    debug!("Browser test result: {test_result}");

    Ok(BrowserOutput {
        output,
        test_result,
        console_logs: Vec::new(),
        exit_code: 0,
    })
}
