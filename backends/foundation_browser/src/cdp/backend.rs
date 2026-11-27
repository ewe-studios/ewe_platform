//! CDP implementation of [`Backend`] (Chromium).
//!
//! Maps the operation set to `Page.*` / `Runtime.*` / `Input.*` over a flattened
//! target session (`sessionId` threaded through every call).

use std::time::Duration;

use base64::Engine as _;
use foundation_context::Ctx;
use serde_json::{json, Value};

use crate::backend::Backend;
use crate::cdp::attach_page;
use crate::error::{BrowserError, Result};
use crate::jsonrpc::{RpcEngine, WireProtocol};

/// A CDP page backend: a borrowed engine + the attached `sessionId`.
pub(crate) struct CdpBackend<'e> {
    engine: &'e RpcEngine,
    session: String,
    ctx: &'e Ctx,
    navigate_timeout: Duration,
}

impl<'e> CdpBackend<'e> {
    /// Create a fresh page target and attach a flattened session to it.
    ///
    /// `navigate_timeout` bounds the wait for the load event of each navigation.
    pub(crate) fn attach(
        engine: &'e RpcEngine,
        ctx: &'e Ctx,
        navigate_timeout: Duration,
    ) -> Result<Self> {
        let session = attach_page(engine)?;
        engine.call(
            "Page.setLifecycleEventsEnabled",
            json!({ "enabled": true }),
            Some(&session),
        )?;
        Ok(Self {
            engine,
            session,
            ctx,
            navigate_timeout,
        })
    }

    fn call(&self, method: &str, params: Value) -> Result<Value> {
        self.engine.call(method, params, Some(&self.session))
    }
}

impl Backend for CdpBackend<'_> {
    fn navigate(&self, url: &str) -> Result<()> {
        // Chromium can send the load event before it answers the command.
        let events = self
            .engine
            .subscribe_session("Page.lifecycleEvent", &self.session);
        let result = self.call("Page.navigate", json!({ "url": url }))?;
        if let Some(error) = result.get("errorText").and_then(Value::as_str) {
            return Err(BrowserError::Protocol {
                code: 0,
                message: format!("Page.navigate {url}: {error}"),
            });
        }
        if result.get("isDownload").and_then(Value::as_bool) == Some(true) {
            return Err(BrowserError::Protocol {
                code: 0,
                message: format!("Page.navigate {url} started a download"),
            });
        }
        let frame = result
            .get("frameId")
            .and_then(Value::as_str)
            .ok_or_else(|| BrowserError::Protocol {
                code: 0,
                message: "Page.navigate returned no frameId".into(),
            })?;
        let Some(loader) = result.get("loaderId").and_then(Value::as_str) else {
            // Same-document navigation keeps the current loader.
            return Ok(());
        };

        let clock = self.ctx.clock();
        let started = clock.now();
        loop {
            let remaining = self
                .navigate_timeout
                .saturating_sub(clock.now().saturating_duration_since(started));
            if let Ok(event) = events.receiver.recv_timeout(remaining) {
                if event.get("name").and_then(Value::as_str) == Some("load")
                    && event.get("frameId").and_then(Value::as_str) == Some(frame)
                    && event.get("loaderId").and_then(Value::as_str) == Some(loader)
                {
                    return Ok(());
                }
                if !remaining.is_zero() {
                    continue;
                }
            }
            if let Some(reason) = self.engine.closed_reason() {
                return Err(BrowserError::ConnectionLost { reason });
            }
            return Err(BrowserError::Timeout {
                op: format!("load {url}"),
                after_ms: u64::try_from(self.navigate_timeout.as_millis()).unwrap_or(u64::MAX),
            });
        }
    }

    fn evaluate(&self, expression: &str) -> Result<Value> {
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
        Ok(res
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    fn screenshot_png(&self) -> Result<Vec<u8>> {
        let res = self.call("Page.captureScreenshot", json!({ "format": "png" }))?;
        let data =
            res.get("data")
                .and_then(Value::as_str)
                .ok_or_else(|| BrowserError::Protocol {
                    code: 0,
                    message: "captureScreenshot returned no data".into(),
                })?;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| BrowserError::Protocol {
                code: 0,
                message: format!("bad base64: {e}"),
            })
    }

    fn click_at(&self, x: f64, y: f64) -> Result<()> {
        for kind in ["mousePressed", "mouseReleased"] {
            self.call(
                "Input.dispatchMouseEvent",
                json!({ "type": kind, "x": x, "y": y, "button": "left", "clickCount": 1 }),
            )?;
        }
        Ok(())
    }

    fn hover_at(&self, x: f64, y: f64) -> Result<()> {
        self.call(
            "Input.dispatchMouseEvent",
            json!({ "type": "mouseMoved", "x": x, "y": y }),
        )?;
        Ok(())
    }

    fn type_text(&self, text: &str) -> Result<()> {
        self.call("Input.insertText", json!({ "text": text }))?;
        Ok(())
    }

    fn press_key(&self, key: &str) -> Result<()> {
        let (code, vk) = cdp_key(key);
        let base = json!({ "key": key, "code": code, "windowsVirtualKeyCode": vk });
        let mut down = base.clone();
        down["type"] = json!("keyDown");
        if key.chars().count() == 1 {
            down["text"] = json!(key);
        }
        self.call("Input.dispatchKeyEvent", down)?;
        let mut up = base;
        up["type"] = json!("keyUp");
        self.call("Input.dispatchKeyEvent", up)?;
        Ok(())
    }
}

/// CDP `code` + `windowsVirtualKeyCode` for common keys (component-test set).
fn cdp_key(key: &str) -> (&'static str, i64) {
    match key {
        "Enter" => ("Enter", 13),
        "Tab" => ("Tab", 9),
        "Escape" => ("Escape", 27),
        " " => ("Space", 32),
        "ArrowUp" => ("ArrowUp", 38),
        "ArrowDown" => ("ArrowDown", 40),
        "ArrowLeft" => ("ArrowLeft", 37),
        "ArrowRight" => ("ArrowRight", 39),
        "Home" => ("Home", 36),
        "End" => ("End", 35),
        "Backspace" => ("Backspace", 8),
        _ => ("", 0),
    }
}
