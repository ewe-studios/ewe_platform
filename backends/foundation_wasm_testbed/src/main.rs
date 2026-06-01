//! CLI entry point for wasm-testbed.
//!
//! Parses CLI arguments, initializes tracing, bootstraps tokio runtime,
//! and dispatches to the appropriate command handler.

use clap::Parser;
use tracing::error;

mod browser;
mod build;
mod cli;
mod deno;
mod init;
mod server;
mod wasm;
mod wasm_test;
mod wrangler;

use cli::{Cli, Command};

fn main() {
    // Initialize tracing subscriber with env-filter for configurable log levels
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let runtime = tokio::runtime::Runtime::new().expect("create tokio runtime");

    let result = match cli.command {
        Command::Init(args) => runtime.block_on(init::run(args)),
        Command::Test(args) => runtime.block_on(run_test(args)),
    };

    if let Err(e) = result {
        error!("{:?}", e);
        std::process::exit(1);
    }
}

/// Dispatch test mode to the appropriate runner.
async fn run_test(args: cli::TestArgs) -> anyhow::Result<()> {
    use cli::Mode;

    let crate_path = std::fs::canonicalize(&args.crate_path)?;

    // Step 1: Build wasm
    tracing::info!("Building wasm...");
    let build = build::run(&crate_path, args.release, args.features.as_deref())?;

    let integration_dir = crate_path.join("integrations").join(args.mode.integration_dir());

    match args.mode {
        Mode::Web => {
            // Copy wasm to integration dir
            let wasm_dest = integration_dir.join(format!("{}.wasm", build.package_name));
            std::fs::copy(&build.wasm_path, &wasm_dest)?;
            tracing::info!("Copied wasm to {}", wasm_dest.display());

            // Start HTTP server
            let server = server::start_serving(&integration_dir)?;
            let url = server.url("index.html");
            tracing::info!("Serving at {}", url);

            // Run browser
            let output = browser::run(&url, &args.browser, args.headless)?;
            tracing::info!("Test output: {}", output.test_result);

            server.shutdown();
            if output.exit_code != 0 {
                anyhow::bail!("Browser test failed");
            }
        }
        Mode::Deno => {
            let wasm_dest = integration_dir.join(format!("{}.wasm", build.package_name));
            std::fs::copy(&build.wasm_path, &wasm_dest)?;
            tracing::info!("Copied wasm to {}", wasm_dest.display());

            let output = deno::run(&integration_dir, "index.js")?;
            if !output.stdout.is_empty() {
                println!("{}", output.stdout);
            }
            if output.exit_code != 0 {
                anyhow::bail!("Deno test failed with exit code {}", output.exit_code);
            }
        }
        Mode::Wrangler => {
            let wasm_dest = integration_dir.join(format!("{}.wasm", build.package_name));
            std::fs::copy(&build.wasm_path, &wasm_dest)?;
            tracing::info!("Copied wasm to {}", wasm_dest.display());

            let output = wrangler::run(&integration_dir, None)?;
            if output.status_code >= 400 {
                anyhow::bail!("Wrangler test failed with status {}", output.status_code);
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

/// Execute the bindgen-web test flow.
fn run_bindgen_web(
    build: &build::BuildOutput,
    integration_dir: &std::path::Path,
    browser: &cli::Browser,
    headless: bool,
) -> anyhow::Result<()> {
    use wasm::BindgenTarget;

    tracing::info!("Running wasm-bindgen (web)...");
    wasm::run_wasm_bindgen(&build.wasm_path, integration_dir, BindgenTarget::Web)?;

    // Generate index.html from template
    init::write_template_to("bindgen-web/index.html", &build.package_name, integration_dir)?;

    // Discover tests
    let bg_wasm = integration_dir.join(format!("{}_bg.wasm", build.package_name));
    let tests = wasm_test::discover_tests(&bg_wasm)?;
    tracing::info!("Discovered {} tests", tests.len());

    // Generate run.js
    init::write_bindgen_runjs_to(
        "bindgen-web/run.js",
        &build.package_name,
        &tests,
        integration_dir,
    )?;

    // Start HTTP server
    let server = server::start_serving(integration_dir)?;
    let url = server.url("index.html");
    tracing::info!("Serving at {}", url);

    let output = browser::run(&url, browser, headless)?;
    tracing::info!("Test output: {}", output.test_result);

    server.shutdown();
    if output.exit_code != 0 {
        anyhow::bail!("Bindgen browser test failed");
    }
    Ok(())
}

/// Execute the bindgen-deno test flow.
fn run_bindgen_deno(
    build: &build::BuildOutput,
    integration_dir: &std::path::Path,
) -> anyhow::Result<()> {
    use wasm::BindgenTarget;

    tracing::info!("Running wasm-bindgen (deno)...");
    wasm::run_wasm_bindgen(&build.wasm_path, integration_dir, BindgenTarget::Deno)?;

    // Discover tests
    let bg_wasm = integration_dir.join(format!("{}_bg.wasm", build.package_name));
    let tests = wasm_test::discover_tests(&bg_wasm)?;
    tracing::info!("Discovered {} tests", tests.len());

    // Generate run.js
    init::write_bindgen_runjs_to("bindgen-deno/run.js", &build.package_name, &tests, integration_dir)?;

    let output = deno::run(integration_dir, "run.js")?;
    if !output.stdout.is_empty() {
        println!("{}", output.stdout);
    }
    if output.exit_code != 0 {
        anyhow::bail!("Bindgen deno test failed with exit code {}", output.exit_code);
    }
    Ok(())
}

/// Execute the bindgen-wrangler test flow.
fn run_bindgen_wrangler(
    build: &build::BuildOutput,
    integration_dir: &std::path::Path,
) -> anyhow::Result<()> {
    use wasm::BindgenTarget;

    tracing::info!("Running wasm-bindgen (esmodules)...");
    wasm::run_wasm_bindgen(&build.wasm_path, integration_dir, BindgenTarget::EsModules)?;

    // Discover tests
    let bg_wasm = integration_dir.join(format!("{}_bg.wasm", build.package_name));
    let tests = wasm_test::discover_tests(&bg_wasm)?;
    tracing::info!("Discovered {} tests", tests.len());

    // Generate worker.js
    init::write_bindgen_runjs_to(
        "bindgen-wrangler/worker.js",
        &build.package_name,
        &tests,
        integration_dir,
    )?;

    // Generate wrangler.toml
    init::write_template_to("bindgen-wrangler/wrangler.toml", &build.package_name, integration_dir)?;

    let output = wrangler::run(integration_dir, None)?;
    if output.status_code >= 400 {
        anyhow::bail!("Bindgen wrangler test failed with status {}", output.status_code);
    }
    println!("{}", output.response_body);
    Ok(())
}
