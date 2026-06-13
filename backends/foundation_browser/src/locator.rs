//! # Locator + assertions (spec-43 phase-1 §5)
//!
//! WHY: Tests address elements by selector and assert on them; the work
//! (resolve a node, read geometry/text/attributes/computed style, dispatch
//! trusted input) is CDP delivered straight from Rust.
//!
//! WHAT: [`Locator`] (element ops) + [`LocatorAssertions`] (retrying assertions).
//!
//! HOW: Resolve the selector to a CDP `nodeId` (`DOM.getDocument` +
//! `DOM.querySelector`); geometry via `DOM.getBoxModel`, attributes via
//! `DOM.getAttributes`, computed style via `CSS.getComputedStyleForNode`, text
//! via a scoped `Runtime.evaluate` (textContent has no dedicated domain), input
//! via `DOM.focus` + `Input.dispatch{Mouse,Key}Event`/`Input.insertText`.
//! Assertions retry to a deadline to absorb reactive timing.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::browser::Page;
use crate::error::{BrowserError, Result};
use crate::geometry::Rect;

/// An element handle addressed by CSS selector (resolved lazily per operation).
pub struct Locator<'p> {
    page: &'p Page<'p>,
    selector: String,
}

impl<'p> Locator<'p> {
    pub(crate) fn new(page: &'p Page<'p>, selector: impl Into<String>) -> Self {
        Self { page, selector: selector.into() }
    }

    /// Resolve the selector to a CDP `nodeId` (errors if nothing matches).
    fn node_id(&self) -> Result<i64> {
        let doc = self.page.call("DOM.getDocument", json!({ "depth": 0 }))?;
        let root = doc
            .get("root")
            .and_then(|r| r.get("nodeId"))
            .and_then(Value::as_i64)
            .ok_or_else(|| BrowserError::Protocol { code: 0, message: "no document root".into() })?;
        let res = self
            .page
            .call("DOM.querySelector", json!({ "nodeId": root, "selector": self.selector }))?;
        match res.get("nodeId").and_then(Value::as_i64) {
            Some(id) if id != 0 => Ok(id),
            _ => Err(BrowserError::SelectorNotFound(self.selector.clone())),
        }
    }

    /// The number of elements matching the selector.
    ///
    /// # Errors
    /// Protocol/evaluate error.
    pub fn count(&self) -> Result<usize> {
        let v = self.page.eval(&format!(
            "document.querySelectorAll({}).length",
            json!(self.selector)
        ))?;
        Ok(usize::try_from(v.as_u64().unwrap_or(0)).unwrap_or(0))
    }

    /// The element's bounding box (viewport pixels).
    ///
    /// # Errors
    /// [`BrowserError::SelectorNotFound`] or a box-model protocol error.
    pub fn bounding_box(&self) -> Result<Rect> {
        let id = self.node_id()?;
        let model = self.page.call("DOM.getBoxModel", json!({ "nodeId": id }))?;
        model
            .get("model")
            .and_then(|m| m.get("content"))
            .and_then(Rect::from_quad)
            .ok_or_else(|| BrowserError::Protocol { code: 0, message: "no box model".into() })
    }

    /// The element's `textContent`.
    ///
    /// # Errors
    /// Protocol/evaluate error.
    pub fn text(&self) -> Result<String> {
        let v = self.page.eval(&format!(
            "(document.querySelector({})||{{}}).textContent || ''",
            json!(self.selector)
        ))?;
        Ok(v.as_str().unwrap_or_default().to_string())
    }

    /// An attribute value, if present.
    ///
    /// # Errors
    /// [`BrowserError::SelectorNotFound`] or a protocol error.
    pub fn attribute(&self, name: &str) -> Result<Option<String>> {
        let id = self.node_id()?;
        let res = self.page.call("DOM.getAttributes", json!({ "nodeId": id }))?;
        // `attributes` is a flat [k, v, k, v, …] array.
        let Some(arr) = res.get("attributes").and_then(Value::as_array) else {
            return Ok(None);
        };
        let mut i = 0;
        while i + 1 < arr.len() {
            if arr[i].as_str() == Some(name) {
                return Ok(arr[i + 1].as_str().map(ToString::to_string));
            }
            i += 2;
        }
        Ok(None)
    }

    /// A computed-style property value.
    ///
    /// # Errors
    /// [`BrowserError::SelectorNotFound`] or a protocol error.
    pub fn computed_style(&self, property: &str) -> Result<String> {
        let id = self.node_id()?;
        let res = self
            .page
            .call("CSS.getComputedStyleForNode", json!({ "nodeId": id }))?;
        let Some(arr) = res.get("computedStyle").and_then(Value::as_array) else {
            return Ok(String::new());
        };
        for entry in arr {
            if entry.get("name").and_then(Value::as_str) == Some(property) {
                return Ok(entry.get("value").and_then(Value::as_str).unwrap_or_default().to_string());
            }
        }
        Ok(String::new())
    }

    /// Whether the element is visible (non-empty box, not `display:none`/
    /// `visibility:hidden`). Returns `Ok(false)` when the selector doesn't match.
    ///
    /// # Errors
    /// Protocol error other than selector-miss.
    pub fn is_visible(&self) -> Result<bool> {
        // Must exist.
        match self.node_id() {
            Ok(_) => {}
            Err(BrowserError::SelectorNotFound(_)) => return Ok(false),
            Err(e) => return Err(e),
        }
        // No box model (display:none / detached) → not rendered → not visible.
        let rect = match self.bounding_box() {
            Ok(rect) => rect,
            Err(BrowserError::Protocol { .. } | BrowserError::SelectorNotFound(_)) => {
                return Ok(false)
            }
            Err(e) => return Err(e),
        };
        if rect.is_empty() {
            return Ok(false);
        }
        let display = self.computed_style("display").unwrap_or_default();
        let visibility = self.computed_style("visibility").unwrap_or_default();
        Ok(display != "none" && visibility != "hidden")
    }

    /// Click the element (trusted mouse press+release at its centre).
    ///
    /// # Errors
    /// [`BrowserError::SelectorNotFound`] or a protocol error.
    pub fn click(&self) -> Result<()> {
        let (x, y) = self.bounding_box()?.center();
        for kind in ["mousePressed", "mouseReleased"] {
            self.page.call(
                "Input.dispatchMouseEvent",
                json!({ "type": kind, "x": x, "y": y, "button": "left", "clickCount": 1 }),
            )?;
        }
        Ok(())
    }

    /// Move the pointer to the element's centre (trusted `mouseMoved`).
    ///
    /// # Errors
    /// [`BrowserError::SelectorNotFound`] or a protocol error.
    pub fn hover(&self) -> Result<()> {
        let (x, y) = self.bounding_box()?.center();
        self.page
            .call("Input.dispatchMouseEvent", json!({ "type": "mouseMoved", "x": x, "y": y }))?;
        Ok(())
    }

    /// Focus the element and insert `text` (does not clear existing content).
    ///
    /// # Errors
    /// [`BrowserError::SelectorNotFound`] or a protocol error.
    pub fn fill(&self, text: &str) -> Result<()> {
        let id = self.node_id()?;
        self.page.call("DOM.focus", json!({ "nodeId": id }))?;
        self.page.call("Input.insertText", json!({ "text": text }))?;
        Ok(())
    }

    /// Press a key while the element is focused (named keys + single chars).
    ///
    /// # Errors
    /// [`BrowserError::SelectorNotFound`] or a protocol error.
    pub fn press(&self, key: &str) -> Result<()> {
        let id = self.node_id()?;
        self.page.call("DOM.focus", json!({ "nodeId": id }))?;
        let (code, vk) = key_codes(key);
        let base = json!({ "key": key, "code": code, "windowsVirtualKeyCode": vk });
        let mut down = base.clone();
        down["type"] = json!("keyDown");
        if key.chars().count() == 1 {
            down["text"] = json!(key);
        }
        self.page.call("Input.dispatchKeyEvent", down)?;
        let mut up = base;
        up["type"] = json!("keyUp");
        self.page.call("Input.dispatchKeyEvent", up)?;
        Ok(())
    }

    /// Start an assertion chain.
    #[must_use]
    pub fn expect(&self) -> LocatorAssertions<'_, 'p> {
        LocatorAssertions { locator: self, deadline: Duration::from_secs(5) }
    }
}

/// `code` + `windowsVirtualKeyCode` for common keys (enough for component tests).
fn key_codes(key: &str) -> (&'static str, i64) {
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

/// Retrying assertions over a [`Locator`]. Each returns `Ok(())` or a
/// [`BrowserError::Assertion`] after the deadline.
pub struct LocatorAssertions<'a, 'p> {
    locator: &'a Locator<'p>,
    deadline: Duration,
}

impl LocatorAssertions<'_, '_> {
    /// Override the retry budget.
    #[must_use]
    pub fn timeout(mut self, budget: Duration) -> Self {
        self.deadline = budget;
        self
    }

    fn retry<F>(&self, what: &str, mut probe: F) -> Result<()>
    where
        F: FnMut() -> Result<(bool, String)>,
    {
        let start = Instant::now();
        loop {
            let actual = match probe() {
                Ok((true, _)) => return Ok(()),
                Ok((false, actual)) => actual,
                Err(BrowserError::SelectorNotFound(_)) => "no element".into(),
                Err(e) => return Err(e),
            };
            if start.elapsed() >= self.deadline {
                return Err(BrowserError::Assertion {
                    selector: self.locator.selector.clone(),
                    expected: what.to_string(),
                    actual,
                });
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Assert the element is visible.
    ///
    /// # Errors
    /// [`BrowserError::Assertion`] if not visible within the budget.
    pub fn to_be_visible(&self) -> Result<()> {
        self.retry("visible", || {
            let v = self.locator.is_visible()?;
            Ok((v, if v { "visible".into() } else { "hidden".into() }))
        })
    }

    /// Assert the element is hidden or absent.
    ///
    /// # Errors
    /// [`BrowserError::Assertion`] if still visible within the budget.
    pub fn to_be_hidden(&self) -> Result<()> {
        self.retry("hidden", || {
            let v = self.locator.is_visible()?;
            Ok((!v, if v { "visible".into() } else { "hidden".into() }))
        })
    }

    /// Assert the trimmed text equals `expected`.
    ///
    /// # Errors
    /// [`BrowserError::Assertion`] on mismatch within the budget.
    pub fn to_have_text(&self, expected: &str) -> Result<()> {
        self.retry(&format!("text == {expected:?}"), || {
            let t = self.locator.text()?;
            Ok((t.trim() == expected, t))
        })
    }

    /// Assert the attribute equals `expected`.
    ///
    /// # Errors
    /// [`BrowserError::Assertion`] on mismatch within the budget.
    pub fn to_have_attribute(&self, name: &str, expected: &str) -> Result<()> {
        self.retry(&format!("[{name}] == {expected:?}"), || {
            let a = self.locator.attribute(name)?;
            Ok((a.as_deref() == Some(expected), a.unwrap_or_else(|| "(absent)".into())))
        })
    }

    /// Assert a computed-style property equals `expected`.
    ///
    /// # Errors
    /// [`BrowserError::Assertion`] on mismatch within the budget.
    pub fn to_have_computed_style(&self, property: &str, expected: &str) -> Result<()> {
        self.retry(&format!("computed {property} == {expected:?}"), || {
            let v = self.locator.computed_style(property)?;
            Ok((v == expected, v))
        })
    }

    /// Assert the selector matches exactly `n` elements.
    ///
    /// # Errors
    /// [`BrowserError::Assertion`] on mismatch within the budget.
    pub fn to_have_count(&self, n: usize) -> Result<()> {
        self.retry(&format!("count == {n}"), || {
            let c = self.locator.count()?;
            Ok((c == n, c.to_string()))
        })
    }
}
