//! BiDi implementation of [`Backend`] (Firefox).
//!
//! Maps the operation set to `browsingContext.*` / `script.*` /
//! `input.performActions`, and converts BiDi `RemoteValue`s back to plain JSON.

use base64::Engine as _;
use serde_json::{json, Map, Value};

use crate::backend::Backend;
use crate::bidi::default_context;
use crate::error::{BrowserError, Result};
use crate::jsonrpc::{RpcEngine, WireProtocol};

/// A BiDi page backend: a borrowed engine + the target browsing-context id.
pub(crate) struct BiDiBackend<'e> {
    engine: &'e RpcEngine,
    context: String,
}

impl<'e> BiDiBackend<'e> {
    /// Target the default browsing context of the (already opened) session.
    pub(crate) fn create(engine: &'e RpcEngine) -> Result<Self> {
        let context = default_context(engine)?;
        Ok(Self { engine, context })
    }

    fn call(&self, method: &str, params: Value) -> Result<Value> {
        self.engine.call(method, params, None)
    }

    fn perform(&self, actions: Value) -> Result<()> {
        self.call(
            "input.performActions",
            json!({ "context": self.context, "actions": actions }),
        )?;
        Ok(())
    }
}

impl Backend for BiDiBackend<'_> {
    fn navigate(&self, url: &str) -> Result<()> {
        self.call(
            "browsingContext.navigate",
            json!({ "context": self.context, "url": url, "wait": "complete" }),
        )?;
        Ok(())
    }

    fn evaluate(&self, expression: &str) -> Result<Value> {
        let res = self.call(
            "script.evaluate",
            json!({
                "expression": expression,
                "target": { "context": self.context },
                "awaitPromise": true,
                "resultOwnership": "none",
                "serializationOptions": { "maxObjectDepth": 10, "maxDomDepth": 0 },
            }),
        )?;
        // EvaluateResult: `{type:"success", result: RemoteValue}` or
        // `{type:"exception", exceptionDetails}` (an uncaught JS error is still a
        // SUCCESSFUL command).
        if res.get("type").and_then(Value::as_str) == Some("success") {
            Ok(remote_value_to_json(res.get("result").unwrap_or(&Value::Null)))
        } else {
            let msg = res
                .get("exceptionDetails")
                .and_then(|e| e.get("text").or_else(|| e.get("message")))
                .and_then(Value::as_str)
                .unwrap_or("script.evaluate exception");
            Err(BrowserError::Protocol { code: 0, message: msg.to_string() })
        }
    }

    fn screenshot_png(&self) -> Result<Vec<u8>> {
        let res =
            self.call("browsingContext.captureScreenshot", json!({ "context": self.context }))?;
        let data = res.get("data").and_then(Value::as_str).ok_or_else(|| BrowserError::Protocol {
            code: 0,
            message: "captureScreenshot returned no data".into(),
        })?;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| BrowserError::Protocol { code: 0, message: format!("bad base64: {e}") })
    }

    fn click_at(&self, x: f64, y: f64) -> Result<()> {
        let (x, y) = (x.round() as i64, y.round() as i64);
        self.perform(json!([{
            "type": "pointer", "id": "mouse", "parameters": { "pointerType": "mouse" },
            "actions": [
                { "type": "pointerMove", "x": x, "y": y },
                { "type": "pointerDown", "button": 0 },
                { "type": "pointerUp", "button": 0 },
            ],
        }]))
    }

    fn hover_at(&self, x: f64, y: f64) -> Result<()> {
        let (x, y) = (x.round() as i64, y.round() as i64);
        self.perform(json!([{
            "type": "pointer", "id": "mouse", "parameters": { "pointerType": "mouse" },
            "actions": [ { "type": "pointerMove", "x": x, "y": y } ],
        }]))
    }

    fn type_text(&self, text: &str) -> Result<()> {
        let mut actions = Vec::new();
        for ch in text.chars() {
            let s = ch.to_string();
            actions.push(json!({ "type": "keyDown", "value": s }));
            actions.push(json!({ "type": "keyUp", "value": s }));
        }
        self.perform(json!([{ "type": "key", "id": "keyboard", "actions": actions }]))
    }

    fn press_key(&self, key: &str) -> Result<()> {
        let value = bidi_key(key);
        self.perform(json!([{
            "type": "key", "id": "keyboard",
            "actions": [
                { "type": "keyDown", "value": value },
                { "type": "keyUp", "value": value },
            ],
        }]))
    }
}

/// Named keys → the WebDriver normalized key values (Unicode PUA); other inputs
/// pass through (single chars type literally).
fn bidi_key(key: &str) -> String {
    let mapped = match key {
        "Enter" => '\u{E007}',
        "Tab" => '\u{E004}',
        "Escape" => '\u{E00C}',
        "ArrowUp" => '\u{E013}',
        "ArrowDown" => '\u{E015}',
        "ArrowLeft" => '\u{E012}',
        "ArrowRight" => '\u{E014}',
        "Home" => '\u{E011}',
        "End" => '\u{E010}',
        "Backspace" => '\u{E003}',
        other => return other.to_string(),
    };
    mapped.to_string()
}

/// Convert a BiDi `RemoteValue` to plain JSON (the shape CDP's `returnByValue`
/// yields), so `Page::eval` is uniform across backends. Handles primitives,
/// arrays/sets, and objects/maps (whose `value` is a list of `[key, value]`
/// pairs). Plain objects returned by our eval snippets serialize fully (DOM
/// objects are excluded via `maxDomDepth: 0`).
fn remote_value_to_json(rv: &Value) -> Value {
    match rv.get("type").and_then(Value::as_str) {
        Some("undefined" | "null") | None => Value::Null,
        Some("string" | "boolean") => rv.get("value").cloned().unwrap_or(Value::Null),
        Some("number") => match rv.get("value") {
            Some(n @ Value::Number(_)) => n.clone(),
            // Special doubles arrive as strings ("NaN"/"Infinity"/"-0").
            Some(Value::String(s)) => s.parse::<f64>().ok().and_then(serde_json::Number::from_f64).map_or(Value::Null, Value::Number),
            _ => Value::Null,
        },
        Some("bigint") => rv
            .get("value")
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<i64>().ok())
            .map_or(Value::Null, Value::from),
        Some("array" | "set") => Value::Array(
            rv.get("value")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(remote_value_to_json).collect())
                .unwrap_or_default(),
        ),
        Some("object" | "map") => {
            let mut obj = Map::new();
            if let Some(pairs) = rv.get("value").and_then(Value::as_array) {
                for pair in pairs {
                    let Some(p) = pair.as_array() else { continue };
                    if p.len() != 2 {
                        continue;
                    }
                    let key = match &p[0] {
                        Value::String(s) => s.clone(),
                        other => remote_value_to_json(other)
                            .as_str()
                            .map_or_else(|| other.to_string(), ToString::to_string),
                    };
                    obj.insert(key, remote_value_to_json(&p[1]));
                }
            }
            Value::Object(obj)
        }
        // RemoteObjectReference without a serialized value, etc.
        _ => rv.get("value").cloned().unwrap_or(Value::Null),
    }
}
