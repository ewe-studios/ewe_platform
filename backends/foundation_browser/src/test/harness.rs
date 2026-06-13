//! # Harness — deterministic setup/teardown + panic trap (spec-43 phase-1 §0)
//!
//! WHY: A browser test owns a server thread + a browser process + a CDP socket.
//! Teardown MUST run whether the body passes, returns `Err`, or panics — and the
//! original outcome must still surface. This is the lifecycle the
//! `#[wasm_ui_server]` macro expands onto.
//!
//! WHAT: [`TestConfig`] + [`Harness`] (`setup` → `run`).
//!
//! HOW: `Harness` owns `driver` then `server` (field declaration order = drop
//! order = teardown order: browser before server). `run` creates a page, runs
//! the body inside `catch_unwind`, and on failure captures a screenshot while
//! the page is still alive before the guards drop (during normal return,
//! `panic!`, or `resume_unwind`) tear everything down.

use std::path::PathBuf;

use crate::browser::{BrowserDriver, Page};
use crate::cdp::launch::{Browser, LaunchConfig};
use crate::error::Result;
use crate::test::server::{PageSource, StaticMount, TestServer};

/// Configuration for a `#[wasm_ui_server]` test.
pub struct TestConfig {
    /// Bind host (default `127.0.0.1`).
    pub host: String,
    /// Bind port (default `0` = ephemeral).
    pub port: u16,
    /// Inline HTML served on `/` (used when `file` is `None`).
    pub html: String,
    /// An HTML file to serve on `/` (takes precedence over `html`).
    pub file: Option<PathBuf>,
    /// A directory of static assets (wasm bundle, JS, CSS) to serve.
    pub static_dir: Option<PathBuf>,
    /// URL prefix for `static_dir` (default `/assets`).
    pub static_mount: String,
    /// Which browser.
    pub browser: Browser,
    /// Run headless (default true).
    pub headless: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 0,
            html: "<!doctype html><html><head><meta charset=utf-8></head><body></body></html>".into(),
            file: None,
            static_dir: None,
            static_mount: "/assets".into(),
            browser: Browser::Chromium,
            headless: true,
        }
    }
}

impl TestConfig {
    fn bind(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    fn launch(&self) -> LaunchConfig {
        LaunchConfig { browser: self.browser, headless: self.headless, extra_args: Vec::new() }
    }

    fn page(&self) -> PageSource {
        match &self.file {
            Some(path) => PageSource::File(path.clone()),
            None => PageSource::Html(self.html.clone()),
        }
    }

    fn statics(&self) -> Vec<StaticMount> {
        self.static_dir
            .as_ref()
            .map(|dir| vec![StaticMount { mount: self.static_mount.clone(), dir: dir.clone() }])
            .unwrap_or_default()
    }
}

/// Owns the server + browser for one test; tears both down on drop.
///
/// Field order is the teardown order: `driver` (stop browser) before `server`.
pub struct Harness {
    driver: BrowserDriver,
    server: TestServer,
}

impl Harness {
    /// Boot the server + browser.
    ///
    /// # Errors
    /// [`crate::BrowserError`] if the server can't bind or the browser can't launch.
    pub fn setup(config: TestConfig) -> Result<Self> {
        let server = TestServer::start(&config.bind(), config.page(), &config.statics())?;
        let driver = BrowserDriver::launch(config.launch())?;
        Ok(Self { driver, server })
    }

    /// The served base URL.
    #[must_use]
    pub fn url(&self) -> String {
        self.server.url()
    }

    /// Run the test body with the server + a page already navigated to it.
    ///
    /// Teardown is deterministic: on `Ok`, scope-drop tears down; on `Err` or a
    /// panic, a failure screenshot is captured (page still alive) and the
    /// outcome is re-raised so the guards still drop and the test still fails.
    ///
    /// # Panics
    /// Panics (failing the test) on setup error, a returned `Err`, or re-raises
    /// a body panic.
    pub fn run<F>(self, name: &str, body: F)
    where
        F: FnOnce(&TestServer, &Page) -> Result<()>,
    {
        let page = match self.driver.new_page() {
            Ok(p) => p,
            Err(e) => panic!("wasm_ui_server: failed to open a page: {e}"),
        };
        if let Err(e) = page.goto(&self.server.url()) {
            panic!("wasm_ui_server: failed to navigate to the test server: {e}");
        }

        let server = &self.server;
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(server, &page)));

        match outcome {
            Ok(Ok(())) => { /* pass — scope drop tears down */ }
            Ok(Err(e)) => {
                capture_failure(&page, name, "error");
                panic!("wasm_ui_server: test returned Err: {e}");
            }
            Err(panic) => {
                capture_failure(&page, name, "panic");
                std::panic::resume_unwind(panic);
            }
        }
    }
}

/// Best-effort failure diagnostics: a screenshot under `target/primal-test/`.
/// Never panics (we're already failing).
fn capture_failure(page: &Page, name: &str, kind: &str) {
    let dir: PathBuf = ["target", "primal-test", name].iter().collect();
    let shot = dir.join(format!("{kind}.png"));
    if let Err(e) = page.screenshot(&shot) {
        tracing::warn!("could not capture failure screenshot: {e}");
    } else {
        tracing::info!("failure screenshot written to {}", shot.display());
    }
}
