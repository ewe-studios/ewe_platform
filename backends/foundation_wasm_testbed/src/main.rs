//! CLI entry point for wasm-testbed.
//!
//! Parses CLI arguments, initializes tracing, and dispatches
//! to the appropriate command handler. All work is synchronous.

use clap::Parser;
use foundation_errstacks::ErrorTrace;
use tracing::error;

mod browser;
mod build;
mod cli;
mod deno;
mod error;
mod init;
mod server;
mod wasm;
mod wasm_test;
mod wrangler;

use cli::{Cli, Command};
use error::WasmTestbedError;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Command::Init(args) => init::run(args),
        Command::Test(args) => run_test(args),
    };

    if let Err(e) = result {
        error!("{:?}", e);
        std::process::exit(1);
    }
}

fn run_test(args: cli::TestArgs) -> Result<(), ErrorTrace<WasmTestbedError>> {
    use cli::Mode;
    use error::{ToTrace, WasmTestbedError};

    let crate_path = std::fs::canonicalize(&args.crate_path)
        .map_err(|e| WasmTestbedError::Io(e).trace())?;

    tracing::info!("Building wasm...");
    let build = build::run(&crate_path, args.release, args.features.as_deref())?;

    let integration_dir = crate_path.join("integrations").join(args.mode.integration_dir());

    match args.mode {
        Mode::Web => {
            let wasm_dest = integration_dir.join(format!("{}.wasm", build.package_name));
            std::fs::copy(&build.wasm_path, &wasm_dest)
                .map_err(|e| WasmTestbedError::Io(e).trace())?;
            tracing::info!("Copied wasm to {}", wasm_dest.display());

            let server = server::start_serving(&integration_dir)?;
            let url = server.url("index.html");
            tracing::info!("Serving at {}", url);

            let output = browser::run(&url, &args.browser, args.headless)?;
            tracing::info!("Test output: {}", output.test_result);

            server.shutdown();
            if output.exit_code != 0 {
                return Err(WasmTestbedError::BrowserTestFailed(output.exit_code, output.test_result).trace());
            }
        }
        Mode::Deno => {
            let wasm_dest = integration_dir.join(format!("{}.wasm", build.package_name));
            std::fs::copy(&build.wasm_path, &wasm_dest)
                .map_err(|e| WasmTestbedError::Io(e).trace())?;
            tracing::info!("Copied wasm to {}", wasm_dest.display());

            let output = deno::run(&integration_dir, "index.js")?;
            if !output.stdout.is_empty() {
                println!("{}", output.stdout);
            }
        }
        Mode::Wrangler => {
            let wasm_dest = integration_dir.join(format!("{}.wasm", build.package_name));
            std::fs::copy(&build.wasm_path, &wasm_dest)
                .map_err(|e| WasmTestbedError::Io(e).trace())?;
            tracing::info!("Copied wasm to {}", wasm_dest.display());

            let output = wrangler::run(&integration_dir, None)?;
            if output.status_code >= 400 {
                return Err(WasmTestbedError::WranglerHttpFailed(
                    format!("status {}", output.status_code)).trace());
            }
            println!("{}", output.response_body);
        }
        Mode::BindgenWeb => {
            run_bindgen_web(&build, &integration_dir, &args.browser, args.headless)?;
        }
        Mode::BindgenDeno => {
            run_bindgen_deno(&build, &integration_dir)?;
        }
        Mode::BindgenWrangler => {
            run_bindgen_wrangler(&build, &integration_dir)?;
        }
    }

    Ok(())
}

fn run_bindgen_web(
    build: &build::BuildOutput,
    integration_dir: &std::path::Path,
    browser: &cli::Browser,
    headless: bool,
) -> Result<(), ErrorTrace<WasmTestbedError>> {
    use error::{ToTrace, WasmTestbedError};
    use wasm::BindgenTarget;

    tracing::info!("Running wasm-bindgen (web)...");
    wasm::run_wasm_bindgen(&build.wasm_path, integration_dir, BindgenTarget::Web)?;

    init::write_template_to("bindgen-web/index.html", &build.package_name, integration_dir)?;

    let bg_wasm = integration_dir.join(format!("{}_bg.wasm", build.package_name));
    let tests = wasm_test::discover_tests(&bg_wasm)?;
    tracing::info!("Discovered {} tests", tests.len());

    init::write_bindgen_runjs_to(
        "bindgen-web/run.js",
        &build.package_name,
        &tests,
        integration_dir,
    )?;

    let server = server::start_serving(integration_dir)?;
    let url = server.url("index.html");
    tracing::info!("Serving at {}", url);

    let output = browser::run(&url, browser, headless)?;
    tracing::info!("Test output: {}", output.test_result);

    server.shutdown();
    if output.exit_code != 0 {
        return Err(WasmTestbedError::BrowserTestFailed(output.exit_code, output.test_result).trace());
    }
    Ok(())
}

fn run_bindgen_deno(
    build: &build::BuildOutput,
    integration_dir: &std::path::Path,
) -> Result<(), ErrorTrace<WasmTestbedError>> {
    use wasm::BindgenTarget;

    tracing::info!("Running wasm-bindgen (deno)...");
    wasm::run_wasm_bindgen(&build.wasm_path, integration_dir, BindgenTarget::Deno)?;

    let bg_wasm = integration_dir.join(format!("{}_bg.wasm", build.package_name));
    let tests = wasm_test::discover_tests(&bg_wasm)?;
    tracing::info!("Discovered {} tests", tests.len());

    init::write_bindgen_runjs_to("bindgen-deno/run.js", &build.package_name, &tests, integration_dir)?;

    let output = deno::run(integration_dir, "run.js")?;
    if !output.stdout.is_empty() {
        println!("{}", output.stdout);
    }
    Ok(())
}

fn run_bindgen_wrangler(
    build: &build::BuildOutput,
    integration_dir: &std::path::Path,
) -> Result<(), ErrorTrace<WasmTestbedError>> {
    use error::{ToTrace, WasmTestbedError};
    use wasm::BindgenTarget;

    tracing::info!("Running wasm-bindgen (esmodules)...");
    wasm::run_wasm_bindgen(&build.wasm_path, integration_dir, BindgenTarget::EsModules)?;

    let bg_wasm = integration_dir.join(format!("{}_bg.wasm", build.package_name));
    let tests = wasm_test::discover_tests(&bg_wasm)?;
    tracing::info!("Discovered {} tests", tests.len());

    init::write_bindgen_runjs_to(
        "bindgen-wrangler/worker.js",
        &build.package_name,
        &tests,
        integration_dir,
    )?;

    init::write_template_to("bindgen-wrangler/wrangler.toml", &build.package_name, integration_dir)?;

    let output = wrangler::run(integration_dir, None)?;
    if output.status_code >= 400 {
        return Err(WasmTestbedError::WranglerHttpFailed(
            format!("status {}", output.status_code)).trace());
    }
    println!("{}", output.response_body);
    Ok(())
}
