//! # BrowserDriver + Page (spec-43 phase-1 §5)
//!
//! WHY: The public entrypoint — launch a browser, open a page, drive it from
//! Rust. Owns the process + engine as RAII so teardown is deterministic.
//!
//! WHAT: [`BrowserDriver`] (process + JSON-RPC engine) and [`Page`] (a target
//! session: `goto`, `eval`, `screenshot`, and — via `locator` — element ops).
//!
//! HOW: `launch` spawns the browser ([`crate::supervisor`]) and connects the
//! engine ([`crate::jsonrpc`]); `new_page` attaches a flattened CDP session.
//! Page commands carry the `sessionId`.

use std::path::Path;
use std::time::Duration;

use base64::Engine as _;
use serde_json::{json, Value};

use crate::cdp::launch::LaunchConfig;
use crate::cdp::attach_page;
use crate::error::{BrowserError, Result};
use crate::jsonrpc::{RpcEngine, WireProtocol};
use crate::supervisor::BrowserProcess;

/// A launched browser: the process + the JSON-RPC engine over its CDP socket.
///
/// Drop order (fields, declaration order): `engine` (close socket / stop reader)
/// then `process` (kill + remove profile) — i.e. session before process.
pub struct BrowserDriver {
    engine: RpcEngine,
    #[allow(dead_code)] // held for its Drop (kills the process)
    process: BrowserProcess,
}

impl BrowserDriver {
    /// Launch a browser and connect the driver.
    ///
    /// # Errors
    /// [`BrowserError`] if the browser is missing, the launch fails, or the CDP
    /// socket can't be reached.
    pub fn launch(config: LaunchConfig) -> Result<Self> {
        let process = BrowserProcess::launch(&config)?;
        let engine = RpcEngine::connect(process.ws_url(), Duration::from_secs(30))?;
        Ok(Self { engine, process })
    }

    /// Open a fresh page (a new CDP target + attached session).
    ///
    /// # Errors
    /// Protocol errors during target creation/attach.
    pub fn new_page(&self) -> Result<Page<'_>> {
        let session = attach_page(&self.engine)?;
        Ok(Page { engine: &self.engine, session })
    }
}

/// A browser page (one CDP target session).
pub struct Page<'d> {
    engine: &'d RpcEngine,
    session: String,
}

impl Page<'_> {
    /// The wire protocol + session — used by [`crate::locator`].
    pub(crate) fn call(&self, method: &str, params: Value) -> Result<Value> {
        self.engine.call(method, params, Some(&self.session))
    }

    /// A [`Locator`](crate::locator::Locator) for `selector`.
    #[must_use]
    pub fn locator<'s>(&'s self, selector: &str) -> crate::locator::Locator<'s> {
        crate::locator::Locator::new(self, selector)
    }

    /// Navigate to `url` and wait for the load event.
    ///
    /// # Errors
    /// Protocol error from `Page.navigate`, or a load timeout.
    pub fn goto(&self, url: &str) -> Result<()> {
        // Subscribe BEFORE navigating so the load event isn't missed.
        let load = self.engine.subscribe("Page.loadEventFired");
        self.call("Page.navigate", json!({ "url": url }))?;
        load.recv_timeout(Duration::from_secs(30)).map_err(|_| BrowserError::Timeout {
            op: format!("load {url}"),
            after_ms: 30_000,
        })?;
        Ok(())
    }

    /// Evaluate a JS expression in the page and return its value (escape hatch).
    ///
    /// # Errors
    /// Protocol error, or a thrown exception surfaced as [`BrowserError::Protocol`].
    pub fn eval(&self, expression: &str) -> Result<Value> {
        let res = self.call(
            "Runtime.evaluate",
            json!({ "expression": expression, "returnByValue": true, "awaitPromise": true }),
        )?;
        if let Some(details) = res.get("exceptionDetails") {
            return Err(BrowserError::Protocol {
                code: 0,
                message: details
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("evaluate threw")
                    .to_string(),
            });
        }
        Ok(res.get("result").and_then(|r| r.get("value")).cloned().unwrap_or(Value::Null))
    }

    /// Capture a PNG screenshot to `path`.
    ///
    /// # Errors
    /// Protocol error, base64/IO error writing the file.
    pub fn screenshot(&self, path: impl AsRef<Path>) -> Result<()> {
        let res = self.call("Page.captureScreenshot", json!({ "format": "png" }))?;
        let data = res
            .get("data")
            .and_then(Value::as_str)
            .ok_or_else(|| BrowserError::Protocol {
                code: 0,
                message: "captureScreenshot returned no data".into(),
            })?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| BrowserError::Protocol { code: 0, message: format!("bad base64: {e}") })?;
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
    /// Complements the retrying [`LocatorAssertions`](crate::LocatorAssertions):
    /// use it to wait for a streamed update to land before reading raw state.
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
    /// JSON object `{ selector: {x,y,width,height} | null }`. Cheaper than one
    /// `bounding_box` call per element when asserting several positions.
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
