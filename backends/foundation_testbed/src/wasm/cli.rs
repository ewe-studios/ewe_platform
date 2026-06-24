//! CLI argument definitions for the wasm-testbed tool.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Wasm compilation target for the testbed.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum WasmTarget {
    /// `wasm32-unknown-unknown` — browser/CF Workers via JS host
    #[default]
    UnknownUnknown,
    /// `wasm32-unknown-emscripten` — browser with emscripten libc + pthreads
    Emscripten,
    /// `wasm32-wasip1` — WASI preview1 (wasmtime/wasmer/Deno)
    Wasip1,
    /// `wasm32-wasip2` — WASI preview2 component model
    Wasip2,
}

impl WasmTarget {
    /// The Rust `--target` triple for `cargo build`.
    #[must_use]
    pub fn triple(self) -> &'static str {
        match self {
            Self::UnknownUnknown => "wasm32-unknown-unknown",
            Self::Emscripten => "wasm32-unknown-emscripten",
            Self::Wasip1 => "wasm32-wasip1",
            Self::Wasip2 => "wasm32-wasip2",
        }
    }
}

/// WASM Testbed — CLI-driven test harness for wasm32-unknown-unknown.
#[derive(Parser)]
#[command(name = "wasm-testbed", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Scaffold integration test directories for a wasm crate
    Init(InitArgs),
    /// Build, stage, and run tests for a wasm crate (legacy/interop modes —
    /// the bindgen-* modes are the explicit wasm-bindgen OPT-IN, decision 031)
    Test(TestArgs),
    /// OWNED default: build → discover `#[wasm_test]` cases → run headless on the
    /// embedded Deno runtime, in-process (requires `wasm-embedded-js`)
    Deno(OwnedRunArgs),
    /// OWNED: same loop served + run under Playwright on our runtime
    Web(OwnedRunArgs),
}

/// Arguments shared by the owned `deno`/`web` runners (features 12/13).
#[derive(Parser)]
pub struct OwnedRunArgs {
    /// Path to the Cargo crate containing `#[wasm_test]` cases
    pub crate_path: PathBuf,

    /// Wasm compilation target (default: unknown-unknown)
    #[arg(long, value_enum, default_value_t)]
    pub target: WasmTarget,

    /// Build in release mode (default: debug)
    #[arg(long)]
    pub release: bool,

    /// Cargo features to enable during build
    #[arg(long)]
    pub features: Option<String>,

    /// Only run cases whose name contains this substring
    #[arg(long)]
    pub filter: Option<String>,

    /// Browser to use (web mode only)
    #[arg(long, default_value = "chrome")]
    pub browser: Browser,

    /// Run the browser headless (web mode only)
    #[arg(long)]
    pub headless: bool,
}

/// Arguments for the `init` subcommand.
#[derive(Parser)]
pub struct InitArgs {
    /// Which integration type to scaffold: web, deno, wrangler
    #[arg(value_enum)]
    pub r#type: Option<InitType>,

    /// Path to the Cargo crate to scaffold
    pub crate_path: PathBuf,
}

/// Integration type for the `init` subcommand.
#[derive(Clone, ValueEnum)]
pub enum InitType {
    /// OWNED `#[wasm_test]` scaffolding (a sample cases file — features 12/13);
    /// run via the embedded Deno runtime with `wasm-testbed deno`
    Owned,
    /// Browser test scaffolding
    Web,
    /// Deno test scaffolding (wasm-bindgen interop)
    Deno,
    /// Cloudflare Workers test scaffolding
    Wrangler,
}

/// Arguments for the `test` subcommand.
#[derive(Parser)]
pub struct TestArgs {
    /// Test mode
    #[arg(value_enum)]
    pub mode: Mode,

    /// Path to the Cargo crate to test
    pub crate_path: PathBuf,

    /// Build in release mode (default: debug)
    #[arg(long)]
    pub release: bool,

    /// Cargo features to enable during build
    #[arg(long)]
    pub features: Option<String>,

    /// Browser to use (web/bindgen-web modes only)
    #[arg(long, default_value = "chrome")]
    pub browser: Browser,

    /// Run browser in headless mode (web/bindgen-web modes only)
    #[arg(long)]
    pub headless: bool,
}

/// Test mode for the `test` subcommand.
#[derive(Clone, ValueEnum)]
pub enum Mode {
    /// Browser test with user's custom harness
    Web,
    /// Deno test with user's custom harness
    Deno,
    /// Cloudflare Workers test with user's custom harness
    Wrangler,
    /// Browser test with auto-generated wasm-bindgen harness
    BindgenWeb,
    /// Deno test with auto-generated wasm-bindgen harness
    BindgenDeno,
    /// Cloudflare Workers test with auto-generated wasm-bindgen harness
    BindgenWrangler,
}

impl Mode {
    /// Whether this mode uses auto-generated wasm-bindgen harness.
    #[must_use]
    pub fn is_bindgen(&self) -> bool {
        matches!(
            self,
            Mode::BindgenWeb | Mode::BindgenDeno | Mode::BindgenWrangler
        )
    }

    /// Returns the integration directory name for this mode.
    #[must_use]
    pub fn integration_dir(&self) -> &str {
        match self {
            Mode::Web => "web",
            Mode::Deno => "deno",
            Mode::Wrangler => "wrangler",
            Mode::BindgenWeb => "bindgen-web",
            Mode::BindgenDeno => "bindgen-deno",
            Mode::BindgenWrangler => "bindgen-wrangler",
        }
    }

    /// Whether this mode requires a browser (Playwright).
    #[must_use]
    pub fn needs_browser(&self) -> bool {
        matches!(self, Mode::Web | Mode::BindgenWeb)
    }

    /// Whether this mode runs via Deno.
    #[must_use]
    pub fn needs_deno(&self) -> bool {
        matches!(self, Mode::Deno | Mode::BindgenDeno)
    }

    /// Whether this mode runs via wrangler dev.
    #[must_use]
    pub fn needs_wrangler(&self) -> bool {
        matches!(self, Mode::Wrangler | Mode::BindgenWrangler)
    }
}

/// Browser selection for web test modes.
#[derive(Clone, Debug, ValueEnum)]
pub enum Browser {
    Chrome,
    Firefox,
    Safari,
}
