//! # The operation-level backend seam (spec-43 phase-2)
//!
//! WHY: `Page`/`Locator` express browser operations (navigate, evaluate, trusted
//! input) without caring whether the wire is CDP (Chromium) or WebDriver BiDi
//! (Firefox). `WireProtocol` abstracts the JSON-RPC *transport*; [`Backend`]
//! abstracts the *operations* on top of it, so one `Page`/`Locator` drives both.
//!
//! WHAT: [`Backend`] — the minimal operation set. All element READS (box, text,
//! attribute, computed style, visibility, count) are layered on `evaluate` in
//! `Locator`, so a backend only implements what genuinely differs: navigation,
//! script evaluation, screenshots, and trusted pointer/keyboard input.
//!
//! HOW: [`crate::cdp::backend::CdpBackend`] maps to `Page.*`/`Runtime.*`/`Input.*`;
//! [`crate::bidi::backend::BiDiBackend`] maps to `browsingContext.*`/`script.*`/
//! `input.performActions`. Both hold a borrowed `&RpcEngine` + their per-page id
//! (CDP `sessionId` / BiDi browsing-context id).

use serde_json::Value;

use crate::error::Result;

/// A browser backend: the protocol-specific operations `Page`/`Locator` need.
///
/// Object-safe (`Page` holds `Box<dyn Backend>`). Coordinate-taking input methods
/// receive viewport CSS pixels (the caller computes them from a bounding box).
pub(crate) trait Backend {
    /// Navigate to `url` and wait for load to complete.
    fn navigate(&self, url: &str) -> Result<()>;

    /// Evaluate a JS expression and return its value as JSON (awaiting promises).
    fn evaluate(&self, expression: &str) -> Result<Value>;

    /// Capture a full-page PNG screenshot (raw bytes).
    fn screenshot_png(&self) -> Result<Vec<u8>>;

    /// Trusted left click (press + release) at viewport `(x, y)`.
    fn click_at(&self, x: f64, y: f64) -> Result<()>;

    /// Trusted pointer move to viewport `(x, y)`.
    fn hover_at(&self, x: f64, y: f64) -> Result<()>;

    /// Trusted text entry into the currently focused element.
    fn type_text(&self, text: &str) -> Result<()>;

    /// Trusted key press (named keys like `Enter`/`ArrowDown`, or a single char).
    fn press_key(&self, key: &str) -> Result<()>;
}
