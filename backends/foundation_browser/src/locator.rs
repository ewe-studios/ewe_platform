//! # Locator + assertions (spec-43 §5)
//!
//! WHY: Tests address elements by selector and assert on them. Reads (box, text,
//! attribute, computed style, visibility, count) run through `Page::eval`, so
//! they are identical on CDP and BiDi; trusted input goes through the protocol
//! [`Backend`](crate::backend::Backend) (CDP `Input.*` / BiDi
//! `input.performActions`).
//!
//! WHAT: [`Locator`] (element ops) + [`LocatorAssertions`] (retrying assertions).
//!
//! HOW: Each read is a small self-contained JS snippet returning a plain value
//! (so both backends serialize it the same); input computes the element centre
//! from its box and dispatches trusted pointer/keyboard events. Assertions retry
//! to a deadline to absorb reactive timing.

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

    /// A JS string literal of the selector, safe to inline in an `eval` snippet.
    fn sel(&self) -> Value {
        json!(self.selector)
    }

    /// The element's box, or `None` when the selector doesn't match.
    fn box_opt(&self) -> Result<Option<Rect>> {
        let v = self.page.eval(&format!(
            "(() => {{ const el=document.querySelector({sel}); if(!el) return null; \
             const r=el.getBoundingClientRect(); \
             return {{x:r.x,y:r.y,width:r.width,height:r.height}}; }})()",
            sel = self.sel()
        ))?;
        if v.is_null() {
            return Ok(None);
        }
        let f = |k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        Ok(Some(Rect { x: f("x"), y: f("y"), width: f("width"), height: f("height") }))
    }

    /// Focus the element; `Err(SelectorNotFound)` if it doesn't match.
    fn focus(&self) -> Result<()> {
        let v = self.page.eval(&format!(
            "(() => {{ const el=document.querySelector({sel}); if(!el) return false; \
             el.focus(); return true; }})()",
            sel = self.sel()
        ))?;
        if v.as_bool() == Some(true) {
            Ok(())
        } else {
            Err(BrowserError::SelectorNotFound(self.selector.clone()))
        }
    }

    /// The number of elements matching the selector.
    ///
    /// # Errors
    /// Protocol/evaluate error.
    pub fn count(&self) -> Result<usize> {
        let v = self.page.eval(&format!("document.querySelectorAll({}).length", self.sel()))?;
        Ok(usize::try_from(v.as_u64().unwrap_or(0)).unwrap_or(0))
    }

    /// The element's bounding box (viewport pixels).
    ///
    /// # Errors
    /// [`BrowserError::SelectorNotFound`] or an evaluate error.
    pub fn bounding_box(&self) -> Result<Rect> {
        self.box_opt()?.ok_or_else(|| BrowserError::SelectorNotFound(self.selector.clone()))
    }

    /// The element's `textContent`.
    ///
    /// # Errors
    /// Protocol/evaluate error.
    pub fn text(&self) -> Result<String> {
        let v = self.page.eval(&format!(
            "(document.querySelector({})||{{}}).textContent || ''",
            self.sel()
        ))?;
        Ok(v.as_str().unwrap_or_default().to_string())
    }

    /// An attribute value, if present.
    ///
    /// # Errors
    /// [`BrowserError::SelectorNotFound`] or an evaluate error.
    pub fn attribute(&self, name: &str) -> Result<Option<String>> {
        let v = self.page.eval(&format!(
            "(() => {{ const el=document.querySelector({sel}); if(!el) return {{found:false}}; \
             return {{found:true, value:el.getAttribute({name})}}; }})()",
            sel = self.sel(),
            name = json!(name)
        ))?;
        if v.get("found").and_then(Value::as_bool) != Some(true) {
            return Err(BrowserError::SelectorNotFound(self.selector.clone()));
        }
        Ok(v.get("value").and_then(Value::as_str).map(ToString::to_string))
    }

    /// A computed-style property value.
    ///
    /// # Errors
    /// [`BrowserError::SelectorNotFound`] or an evaluate error.
    pub fn computed_style(&self, property: &str) -> Result<String> {
        let v = self.page.eval(&format!(
            "(() => {{ const el=document.querySelector({sel}); if(!el) return null; \
             return getComputedStyle(el).getPropertyValue({prop}); }})()",
            sel = self.sel(),
            prop = json!(property)
        ))?;
        if v.is_null() {
            return Err(BrowserError::SelectorNotFound(self.selector.clone()));
        }
        Ok(v.as_str().unwrap_or_default().trim().to_string())
    }

    /// Whether the element is visible (non-empty box, not `display:none`/
    /// `visibility:hidden`). `Ok(false)` when the selector doesn't match.
    ///
    /// # Errors
    /// Evaluate error.
    pub fn is_visible(&self) -> Result<bool> {
        let v = self.page.eval(&format!(
            "(() => {{ const el=document.querySelector({sel}); if(!el) return false; \
             const r=el.getBoundingClientRect(); const s=getComputedStyle(el); \
             return r.width>0 && r.height>0 && s.display!=='none' && s.visibility!=='hidden'; }})()",
            sel = self.sel()
        ))?;
        Ok(v.as_bool().unwrap_or(false))
    }

    /// Click the element (trusted mouse press+release at its centre).
    ///
    /// # Errors
    /// [`BrowserError::SelectorNotFound`] or a protocol error.
    pub fn click(&self) -> Result<()> {
        let (x, y) = self.bounding_box()?.center();
        self.page.backend().click_at(x, y)
    }

    /// Move the pointer to the element's centre (trusted).
    ///
    /// # Errors
    /// [`BrowserError::SelectorNotFound`] or a protocol error.
    pub fn hover(&self) -> Result<()> {
        let (x, y) = self.bounding_box()?.center();
        self.page.backend().hover_at(x, y)
    }

    /// Focus the element and type `text` (does not clear existing content).
    ///
    /// # Errors
    /// [`BrowserError::SelectorNotFound`] or a protocol error.
    pub fn fill(&self, text: &str) -> Result<()> {
        self.focus()?;
        self.page.backend().type_text(text)
    }

    /// Press a key while the element is focused (named keys + single chars).
    ///
    /// # Errors
    /// [`BrowserError::SelectorNotFound`] or a protocol error.
    pub fn press(&self, key: &str) -> Result<()> {
        self.focus()?;
        self.page.backend().press_key(key)
    }

    /// Start an assertion chain.
    #[must_use]
    pub fn expect(&self) -> LocatorAssertions<'_, 'p> {
        LocatorAssertions { locator: self, deadline: Duration::from_secs(5) }
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
