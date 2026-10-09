//! # Errors (spec-43 phase 1 §7)
//!
//! WHY: A browser driver fails in a handful of distinct, actionable ways —
//! the browser isn't installed, the launch/connect failed, the protocol
//! returned an error, an operation timed out, a selector/assertion didn't hold.
//! Each maps to a clear message the test author can act on.
//!
//! WHAT: [`BrowserError`] (the variants) + [`Result`] alias.
//!
//! HOW: `derive_more` for `Display`/`Error`/`From`; the
//! [`BrowserError::BrowserNotInstalled`] variant carries the `mise` task to run.

use derive_more::derive::{Display, Error, From};

/// The result type used throughout the driver.
pub type Result<T> = core::result::Result<T, BrowserError>;

/// A browser-driver error.
#[derive(Debug, Display, Error, From)]
pub enum BrowserError {
    /// Failed to connect the JSON-RPC WebSocket to the browser.
    #[display("failed to connect to the browser debugging socket: {_0}")]
    #[from(ignore)]
    Connect(#[error(not(source))] String),

    /// Failed to launch the browser process.
    #[display("failed to launch the browser: {_0}")]
    #[from(ignore)]
    Launch(#[error(not(source))] String),

    /// The browser binary was not found.
    #[display("browser '{binary}' not found — install it with `{mise_task}` or set its *_BIN env var")]
    BrowserNotInstalled {
        /// The binary we looked for (e.g. `chromium`).
        binary: String,
        /// The `mise` task that installs it.
        mise_task: String,
    },

    /// The browser returned a protocol-level error for a command.
    #[display("protocol error {code}: {message}")]
    Protocol {
        /// CDP error code.
        code: i64,
        /// CDP error message.
        message: String,
    },

    /// An operation exceeded its deadline.
    #[display("operation '{op}' timed out after {after_ms}ms")]
    Timeout {
        /// The operation that timed out.
        op: String,
        /// The elapsed budget in milliseconds.
        after_ms: u64,
    },

    /// A selector matched no element.
    #[display("selector matched no element: {_0}")]
    #[from(ignore)]
    SelectorNotFound(#[error(not(source))] String),

    /// An assertion did not hold.
    #[display("assertion failed for `{selector}`: expected {expected}, got {actual}")]
    Assertion {
        /// The locator selector.
        selector: String,
        /// What was expected.
        expected: String,
        /// What was observed.
        actual: String,
    },

    /// An I/O error (process spawn, file write, …).
    #[display("io error: {_0}")]
    Io(std::io::Error),

    /// A JSON (de)serialization error.
    #[display("json error: {_0}")]
    Json(serde_json::Error),
}
