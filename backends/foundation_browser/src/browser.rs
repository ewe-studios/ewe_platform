//! # BrowserDriver + Page (spec-43 §5)
//!
//! WHY: The public entrypoint — launch a browser, open a page, drive it from
//! Rust. Owns the process + engine as RAII so teardown is deterministic.
//!
//! WHAT: [`BrowserDriver`] (process + JSON-RPC engine) and [`Page`] (a backend
//! session: `goto`, `eval`, `screenshot`, and — via `locator` — element ops).
//!
//! HOW: `launch` spawns the browser ([`crate::supervisor`]) and connects the
//! engine ([`crate::jsonrpc`]); `new_page` builds the protocol [`Backend`] —
//! CDP (Chromium) or BiDi (Firefox) — behind one `Page` API.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::backend::Backend;
use crate::bidi::backend::BiDiBackend;
use crate::cdp::backend::CdpBackend;
use crate::cdp::launch::{Browser, LaunchConfig};
use crate::error::Result;
use crate::jsonrpc::RpcEngine;
use crate::supervisor::BrowserProcess;

/// A launched browser: the process + the JSON-RPC engine over its socket.
///
/// Drop order (fields, declaration order): `engine` (close socket / stop reader)
/// then `process` (kill + remove profile) — i.e. session before process.
pub struct BrowserDriver {
    engine: RpcEngine,
    browser: Browser,
    #[allow(dead_code)] // held for its Drop (kills the process)
    process: BrowserProcess,
}

impl BrowserDriver {
    /// Launch a browser and connect the driver.
    ///
    /// # Errors
    /// [`BrowserError`] if the browser is missing, the launch fails, or the
    /// CDP/BiDi socket can't be reached.
    pub fn launch(config: LaunchConfig) -> Result<Self> {
        let browser = config.browser;
        let process = BrowserProcess::launch(&config)?;
        // Firefox's remote agent takes a beat to start listening; CDP's endpoint
        // is ready as soon as DevToolsActivePort appears, so one shot suffices.
        let engine = connect_with_retry(process.ws_url(), browser)?;
        // BiDi is connection-scoped: open the session once, up front.
        if browser == Browser::Firefox {
            crate::bidi::new_session(&engine)?;
        }
        Ok(Self { engine, browser, process })
    }

    /// Open a fresh page (a CDP target session, or a BiDi browsing context).
    ///
    /// # Errors
    /// Protocol errors during session/context setup.
    pub fn new_page(&self) -> Result<Page<'_>> {
        let backend: Box<dyn Backend + '_> = match self.browser {
            Browser::Chromium => Box::new(CdpBackend::attach(&self.engine)?),
            Browser::Firefox => Box::new(BiDiBackend::create(&self.engine)?),
        };
        Ok(Page { backend })
    }
}

/// Connect the engine, retrying briefly (Firefox needs a moment to listen).
fn connect_with_retry(ws_url: &str, browser: Browser) -> Result<RpcEngine> {
    let call_timeout = Duration::from_secs(30);
    if browser == Browser::Chromium {
        return RpcEngine::connect(ws_url, call_timeout);
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match RpcEngine::connect(ws_url, call_timeout) {
            Ok(engine) => return Ok(engine),
            Err(e) if Instant::now() >= deadline => return Err(e),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// A browser page (one CDP target session or BiDi browsing context).
pub struct Page<'d> {
    backend: Box<dyn Backend + 'd>,
}

impl Page<'_> {
    /// The protocol backend — used by [`crate::locator`] for trusted input.
    pub(crate) fn backend(&self) -> &dyn Backend {
        self.backend.as_ref()
    }

    /// A [`Locator`](crate::locator::Locator) for `selector`.
    #[must_use]
    pub fn locator<'s>(&'s self, selector: &str) -> crate::locator::Locator<'s> {
        crate::locator::Locator::new(self, selector)
    }

    /// Navigate to `url` and wait for load to complete.
    ///
    /// # Errors
    /// Protocol error or a load timeout.
    pub fn goto(&self, url: &str) -> Result<()> {
        self.backend.navigate(url)
    }

    /// Evaluate a JS expression in the page and return its value (escape hatch).
    ///
    /// # Errors
    /// Protocol error, or a thrown exception surfaced as [`BrowserError::Protocol`].
    pub fn eval(&self, expression: &str) -> Result<Value> {
        self.backend.evaluate(expression)
    }

    /// Capture a PNG screenshot to `path`.
    ///
    /// # Errors
    /// Protocol error, or an IO error writing the file.
    pub fn screenshot(&self, path: impl AsRef<Path>) -> Result<()> {
        let bytes = self.backend.screenshot_png()?;
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Wait until the server-driven DOM has SETTLED — no applied frame for
    /// `quiet_ms` — or `timeout_ms` elapses. Reads the runtime's always-on frame
    /// instrument (`globalThis.__primalFrames`, bumped by `Patcher.route`), so it
    /// needs no in-page helper. Returns `true` if it settled, `false` on timeout.
    ///
    /// # Errors
    /// Protocol error from the evaluate call.
    pub fn wait_for_reactive(&self, quiet_ms: u64, timeout_ms: u64) -> Result<bool> {
        let step = quiet_ms.min(25).max(1);
        let expr = format!(
            "(async () => {{ const t=()=>performance.now(); const start=t(); \
             for(;;) {{ const f=globalThis.__primalFrames; \
             if (f && t()-f.at >= {quiet_ms}) return true; \
             if (t()-start > {timeout_ms}) return false; \
             await new Promise(r=>setTimeout(r,{step})); }} }})()"
        );
        Ok(self.eval(&expr)?.as_bool().unwrap_or(false))
    }

    /// Read the bounding boxes of many selectors in ONE layout pass. Returns a
    /// JSON object `{ selector: {x,y,width,height} | null }`.
    ///
    /// # Errors
    /// Protocol error from the evaluate call.
    pub fn bounding_boxes(&self, selectors: &[&str]) -> Result<Value> {
        let list = serde_json::to_string(selectors).unwrap_or_else(|_| "[]".into());
        let expr = format!(
            "Object.fromEntries({list}.map(s => {{ const el=document.querySelector(s); \
             if(!el) return [s,null]; const r=el.getBoundingClientRect(); \
             return [s,{{x:r.x,y:r.y,width:r.width,height:r.height}}]; }}))"
        );
        self.eval(&expr)
    }
}
