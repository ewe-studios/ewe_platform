//! Error types for the wasm testbed.
//!
//! WHY: The project uses `foundation_errstacks` for error handling — not `anyhow`.
//! WHAT: A single enum covering all failure modes across the testbed.
//! HOW: `derive_more` provides Display/Error, we manually impl `From` where needed.

use derive_more::{Display, Error};
use foundation_errstacks::ErrorTrace;

/// All error variants for the wasm testbed CLI.
#[derive(Debug, Display, Error)]
pub enum WasmTestbedError {
    // --- init.rs ---
    #[display("Crate path does not exist: {_0}")]
    #[error(ignore)]
    CratePathNotFound(String),

    #[display("Not a Cargo crate: {_0}")]
    #[error(ignore)]
    NotACargoCrate(String),

    #[display("Cargo.toml missing [package].name in {_0}")]
    #[error(ignore)]
    MissingPackageName(String),

    #[display("Template not found: {_0}")]
    #[error(ignore)]
    TemplateNotFound(String),

    #[display("Template is not valid UTF-8: {_0}")]
    TemplateNotUtf8(std::string::FromUtf8Error),

    #[display("Invalid template path: {_0}")]
    #[error(ignore)]
    InvalidTemplatePath(String),

    // --- build.rs ---
    #[display("Cargo not found on PATH")]
    #[error(ignore)]
    CargoNotFound,

    #[display("Failed to execute cargo: {_0}")]
    CargoExecFailed(std::io::Error),

    #[display(
        "cargo build failed (exit code {_0:?})\n\
        Hint: if wasm32-unknown-unknown target is not installed, run:\n\
        rustup target add wasm32-unknown-unknown"
    )]
    #[error(ignore)]
    CargoBuildFailed(Option<i32>),

    #[display("Wasm binary not found at {_0}\nExpected cargo build to produce this file.")]
    #[error(ignore)]
    WasmBinaryNotFound(String),

    // --- wasm.rs ---
    #[display(
        "wasm-bindgen not found on PATH.\n\
        Install: cargo install wasm-bindgen-cli"
    )]
    #[error(ignore)]
    WasmBindgenNotFound,

    #[display("Failed to execute wasm-bindgen: {_0}")]
    WasmBindgenExecFailed(std::io::Error),

    #[display("wasm-bindgen failed (exit code {_0:?})")]
    #[error(ignore)]
    WasmBindgenFailed(Option<i32>),

    // --- wasm_test.rs ---
    #[display("Wasm binary not found: {_0}")]
    #[error(ignore)]
    TestWasmNotFound(String),

    #[display("Failed to read wasm binary: {_0}")]
    WasmReadFailed(std::io::Error),

    #[display("Failed to parse wasm binary: {_0}")]
    #[error(ignore)]
    WasmParseFailed(String),

    // --- deno.rs ---
    #[display(
        "deno not found on PATH.\n\
        Install from https://deno.land"
    )]
    #[error(ignore)]
    DenoNotFound,

    #[display("Entry file not found: {_0}\nHint: run wasm-testbed init deno ./crate first.")]
    #[error(ignore)]
    DenoEntryNotFound(String),

    #[display("Failed to execute deno: {_0}")]
    DenoExecFailed(std::io::Error),

    #[display(
        "deno test failed (exit code {_0})\n\
        stdout:\n{_1}\n\
        stderr:\n{_2}"
    )]
    #[error(ignore)]
    DenoTestFailed(i32, String, String),

    // --- wrangler.rs ---
    #[display(
        "wrangler not found on PATH.\n\
        Install: npm install -g wrangler"
    )]
    #[error(ignore)]
    WranglerNotFound,

    #[display("No available port found")]
    #[error(ignore)]
    NoPortAvailable,

    #[display("Failed to start wrangler: {_0}")]
    WranglerStartFailed(std::io::Error),

    #[display(
        "wrangler dev did not become ready within 10 seconds\n\
        wrangler stderr:\n{_0}"
    )]
    #[error(ignore)]
    WranglerTimeout(String),

    #[display("HTTP request to wrangler failed: {_0}")]
    #[error(ignore)]
    WranglerHttpFailed(String),

    // --- browser.rs ---
    #[display(
        "node not found on PATH.\n\
        Playwright requires Node.js."
    )]
    #[error(ignore)]
    NodeNotFound,

    #[display("Failed to initialize npm package for playwright")]
    #[error(ignore)]
    NpmInitFailed,

    #[display("Failed to install playwright")]
    #[error(ignore)]
    PlaywrightInstallFailed,

    #[display(
        "Failed to install playwright browser: {_0}\n\
        Run: npx playwright install {_0}"
    )]
    #[error(ignore)]
    PlaywrightBrowserInstallFailed(String),

    #[display(
        "Browser test failed (exit code {_0})\n\
        Test result: {_1}"
    )]
    #[error(ignore)]
    BrowserTestFailed(i32, String),

    // --- server.rs ---
    #[display("Failed to start HTTP server: {_0}")]
    #[error(ignore)]
    ServerStartFailed(String),

    // --- generic ---
    #[display("I/O error: {_0}")]
    Io(std::io::Error),
}

/// Shorthand for Result<T, ErrorTrace<WasmTestbedError>>.
pub type Result<T> = std::result::Result<T, ErrorTrace<WasmTestbedError>>;

/// Extension trait to convert a plain error into an ErrorTrace.
pub trait ToTrace {
    fn trace(self) -> ErrorTrace<WasmTestbedError>;
}

impl ToTrace for WasmTestbedError {
    fn trace(self) -> ErrorTrace<WasmTestbedError> {
        ErrorTrace::new(self)
    }
}

impl From<std::io::Error> for WasmTestbedError {
    fn from(e: std::io::Error) -> Self {
        WasmTestbedError::Io(e)
    }
}
