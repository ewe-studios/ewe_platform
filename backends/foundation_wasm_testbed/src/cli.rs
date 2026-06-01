//! CLI argument definitions for the wasm-testbed tool.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

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
    /// Build, stage, and run tests for a wasm crate
    Test(TestArgs),
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
    /// Browser test scaffolding
    Web,
    /// Deno test scaffolding
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
    pub fn is_bindgen(&self) -> bool {
        matches!(
            self,
            Mode::BindgenWeb | Mode::BindgenDeno | Mode::BindgenWrangler
        )
    }

    /// Returns the integration directory name for this mode.
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
    pub fn needs_browser(&self) -> bool {
        matches!(self, Mode::Web | Mode::BindgenWeb)
    }

    /// Whether this mode runs via Deno.
    pub fn needs_deno(&self) -> bool {
        matches!(self, Mode::Deno | Mode::BindgenDeno)
    }

    /// Whether this mode runs via wrangler dev.
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
