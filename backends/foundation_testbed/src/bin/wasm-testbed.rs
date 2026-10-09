//! CLI entry point for wasm-testbed.
//!
//! Parses CLI arguments, initializes tracing, and dispatches
//! to the appropriate command handler. All work is synchronous.

#![cfg(feature = "wasm")]

use clap::Parser;
use foundation_errstacks::ErrorTrace;
use tracing::error;

use foundation_testbed::wasm::{
    browser, build, cli, deno, emscripten_runner, error, fwt_runner, init, server, wasi_runner,
    wasm, wasm_test, wrangler,
};
#[cfg(feature = "wasm-bindgen-test")]
use foundation_testbed::wasm::bindgen_runner;

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
        Command::Test(args) => run_test(&args),
        Command::Deno(args) => run_owned(fwt_runner::run_deno(&args)),
        Command::Browser(args) => run_owned(fwt_runner::run_web(&args)),
        Command::Bindgen(args) => {
            #[cfg(feature = "wasm-bindgen-test")]
            {
                run_bindgen_outcome(bindgen_runner::run_bindgen(&args))
            }
            #[cfg(not(feature = "wasm-bindgen-test"))]
            {
                use error::ToTrace;
                Err(WasmTestbedError::BrowserDriver(
                    "the `bindgen` command requires the `wasm-bindgen-test` feature.\n\
                     Re-run with: cargo run -p foundation_testbed \
                     --features wasm,wasm-bindgen-test --bin wasm-testbed -- bindgen ..."
                        .into(),
                )
                .trace())
            }
        }
        Command::Emscripten(args) => run_owned(emscripten_runner::run(&args)),
        Command::Wasi(args) => run_subprocess(wasi_runner::run(&args)),
    };

    if let Err(e) = result {
        error!("{:?}", e);
        std::process::exit(1);
    }
}

/// Surface a subprocess runner (emscripten/wasi) outcome as the process verdict.
fn run_subprocess<T>(
    outcome: foundation_testbed::wasm::error::Result<T>,
) -> Result<(), ErrorTrace<WasmTestbedError>> {
    outcome?;
    Ok(())
}

/// Surface an owned-harness outcome as the process verdict.
fn run_owned(
    outcome: foundation_testbed::wasm::error::Result<fwt_runner::RunOutcome>,
) -> Result<(), ErrorTrace<WasmTestbedError>> {
    use error::ToTrace;
    let outcome = outcome?;
    if outcome.exit_code != 0 {
        return Err(WasmTestbedError::OwnedRunFailed(outcome.exit_code).trace());
    }
    Ok(())
}

/// Surface a bindgen browser outcome as the process verdict.
#[cfg(feature = "wasm-bindgen-test")]
fn run_bindgen_outcome(
    outcome: foundation_testbed::wasm::error::Result<bindgen_runner::RunOutcome>,
) -> Result<(), ErrorTrace<WasmTestbedError>> {
    use error::ToTrace;
    let outcome = outcome?;
    if outcome.exit_code != 0 {
        return Err(WasmTestbedError::OwnedRunFailed(outcome.exit_code).trace());
    }
    Ok(())
}

fn run_test(args: &cli::TestArgs) -> Result<(), ErrorTrace<WasmTestbedError>> {
    use cli::Mode;
    use error::{ToTrace, WasmTestbedError};

    let crate_path =
        std::fs::canonicalize(&args.crate_path).map_err(|e| WasmTestbedError::Io(e).trace())?;

    let integration_dir = crate_path
        .join("integrations")
        .join(args.mode.integration_dir());

    match args.mode {
        Mode::Web | Mode::Deno | Mode::Wrangler => {
            // Custom harness modes: build library only, user's JS handles testing
            let build = build::run(&crate_path, args.release, args.features.as_deref())?;
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
                        return Err(WasmTestbedError::BrowserTestFailed(
                            output.exit_code,
                            output.test_result,
                        )
                        .trace());
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
                        return Err(WasmTestbedError::WranglerHttpFailed(format!(
                            "status {}",
                            output.status_code
                        ))
                        .trace());
                    }
                    println!("{}", output.response_body);
                }
                _ => unreachable!(),
            }
        }
        Mode::BindgenWeb | Mode::BindgenDeno | Mode::BindgenWrangler => {
            // F14 boundary: this is the EXPLICIT wasm-bindgen interop opt-in — not
            // the owned default. Our crates test via `wasm-testbed deno/web`.
            tracing::warn!(
                "INTEROP MODE: running the wasm-bindgen (non-owned) path — \
                 sanctioned only for crates that themselves use wasm-bindgen \
                 (decision 031 / feature 14)"
            );
            // Bindgen modes: need --tests for #[wasm_bindgen_test] exports
            let build = build::run_with_tests(&crate_path, args.release, args.features.as_deref())?;
            match args.mode {
                Mode::BindgenWeb => {
                    run_bindgen_web(&build, &integration_dir, &args.browser, args.headless)?;
                }
                Mode::BindgenDeno => {
                    run_bindgen_deno(&build, &integration_dir)?;
                }
                Mode::BindgenWrangler => {
                    run_bindgen_wrangler(&build, &integration_dir)?;
                }
                _ => unreachable!(),
            }
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
    // Pass --out-name so the output is {name}.js / {name}_bg.wasm (no hash).
    // The deno mode already does this; without it, the hashed input filename
    // becomes the output name and the index.html/run.js imports don't resolve.
    wasm::run_wasm_bindgen_with_name(
        &build.wasm_path,
        integration_dir,
        BindgenTarget::Web,
        Some(&build.package_name),
    )?;

    init::write_template_to(
        "bindgen-web/index.html",
        &build.package_name,
        integration_dir,
    )?;

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
        return Err(
            WasmTestbedError::BrowserTestFailed(output.exit_code, output.test_result).trace(),
        );
    }
    Ok(())
}

fn run_bindgen_deno(
    build: &build::BuildOutput,
    integration_dir: &std::path::Path,
) -> Result<(), ErrorTrace<WasmTestbedError>> {
    use error::{ToTrace, WasmTestbedError};
    use wasm::BindgenTarget;

    tracing::info!("Running wasm-bindgen (deno)...");
    wasm::run_wasm_bindgen_with_name(
        &build.wasm_path,
        integration_dir,
        BindgenTarget::Deno,
        Some(&build.package_name),
    )?;

    // Patch the generated JS to export the `wasm` variable (wasm exports).
    // The deno target generates `const wasm = wasmInstance.exports;` locally;
    // we need it exported so run.js can access __wbgt_ test functions.
    let js_file = integration_dir.join(format!("{}.js", build.package_name));
    if let Ok(content) = std::fs::read_to_string(&js_file) {
        // Add `export { wasm };` at the end of the file
        let patched = format!("{content}\nexport {{ wasm }};\n");
        std::fs::write(&js_file, patched).map_err(|e| WasmTestbedError::Io(e).trace())?;
    }

    let bg_wasm = integration_dir.join(format!("{}_bg.wasm", build.package_name));
    let tests = wasm_test::discover_tests(&bg_wasm)?;
    tracing::info!("Discovered {} tests", tests.len());

    init::write_bindgen_runjs_to(
        "bindgen-deno/run.js",
        &build.package_name,
        &tests,
        integration_dir,
    )?;

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
    wasm::run_wasm_bindgen_with_name(
        &build.wasm_path,
        integration_dir,
        BindgenTarget::EsModules,
        Some(&build.package_name),
    )?;

    let bg_wasm = integration_dir.join(format!("{}_bg.wasm", build.package_name));
    let tests = wasm_test::discover_tests(&bg_wasm)?;
    tracing::info!("Discovered {} tests", tests.len());

    init::write_bindgen_runjs_to(
        "bindgen-wrangler/worker.js",
        &build.package_name,
        &tests,
        integration_dir,
    )?;

    init::write_template_to(
        "bindgen-wrangler/wrangler.toml",
        &build.package_name,
        integration_dir,
    )?;

    let output = wrangler::run(integration_dir, None)?;
    if output.status_code >= 400 {
        return Err(
            WasmTestbedError::WranglerHttpFailed(format!("status {}", output.status_code)).trace(),
        );
    }
    println!("{}", output.response_body);
    Ok(())
}
