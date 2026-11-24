//! CLI entry point for wasm-testbed.
//!
//! Parses CLI arguments, initializes tracing, and dispatches
//! to the appropriate command handler. All work is synchronous.

#![cfg(feature = "wasm")]

use clap::Parser;
use foundation_errstacks::ErrorTrace;
use tracing::error;

#[cfg(feature = "wasm-bindgen-test")]
use foundation_testbed::wasm::bindgen_runner;
use foundation_testbed::wasm::{
    browser, build, cli, deno, emscripten_runner, error, fwt_runner, init, server, wasi_runner,
    wasm, wasm_test, wrangler,
};

use cli::{Cli, Command};
use error::WasmTestbedError;

fn main() {
    let mut filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    // PRIMAL_CDP_TRACE=1 prints the CDP wire: every frame the engine sends and
    // every frame the browser answers. Use it to diagnose a stalled page.
    if std::env::var("PRIMAL_CDP_TRACE").is_ok_and(|value| value == "1")
        && let Ok(directive) = "foundation_browser=trace".parse()
    {
        filter = filter.add_directive(directive);
    }
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let cli = Cli::parse();

    let result = match cli.command {
        Command::Init(args) => init::run(args),
        Command::Test(args) => run_test(&args),
        Command::Deno(args) => run_owned(fwt_runner::run_deno(&args)),
        Command::Browser(args) => {
            run_owned(fwt_runner::run_web(&args, &foundation_context::Ctx::host()))
        }
        Command::Bindgen(args) => run_bindgen_command(&args),
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
        return Err(WasmTestbedError::OwnedRunFailed {
            exit_code: outcome.exit_code,
            output: outcome.output,
        }
        .trace());
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
        return Err(WasmTestbedError::OwnedRunFailed {
            exit_code: outcome.exit_code,
            output: outcome.output,
        }
        .trace());
    }
    Ok(())
}

/// Run the bindgen command. The `args` binding lives behind the same feature
/// gate as its use, so the other arm never warns about an unused argument.
#[cfg(feature = "wasm-bindgen-test")]
fn run_bindgen_command(args: &cli::BindgenArgs) -> Result<(), ErrorTrace<WasmTestbedError>> {
    run_bindgen_outcome(bindgen_runner::run_bindgen(
        args,
        &foundation_context::Ctx::host(),
    ))
}

#[cfg(not(feature = "wasm-bindgen-test"))]
fn run_bindgen_command(_args: &cli::BindgenArgs) -> Result<(), ErrorTrace<WasmTestbedError>> {
    use error::ToTrace;
    Err(WasmTestbedError::BrowserDriver(
        "the `bindgen` command requires the `wasm-bindgen-test` feature.\n\
         Re-run with: cargo run -p foundation_testbed \
         --features wasm,wasm-bindgen-test --bin wasm-testbed -- bindgen ..."
            .into(),
    )
    .trace())
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
                    let output = browser::run(
                        &url,
                        &args.browser,
                        args.headless,
                        &foundation_context::Ctx::host(),
                        &browser::BrowserRunLimits::default(),
                    )?;
                    tracing::info!("Test output: {}", output.output);
                    server.shutdown();
                    if output.exit_code != 0 {
                        return Err(WasmTestbedError::BrowserTestFailed(
                            output.exit_code,
                            output.output,
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
                    let output =
                        wrangler::run(&integration_dir, None, &foundation_context::Ctx::host())?;
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
                    run_bindgen_web(
                        &build,
                        &integration_dir,
                        &args.browser,
                        args.headless,
                        args.filter.as_deref(),
                    )?;
                }
                Mode::BindgenDeno => {
                    run_bindgen_deno(&build, &integration_dir, args.filter.as_deref())?;
                }
                Mode::BindgenWrangler => {
                    run_bindgen_wrangler(&build, &integration_dir, args.filter.as_deref())?;
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
    filter: Option<&str>,
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

    // The page imports the glue's `init`, which returns the wasm exports, so
    // the glue does not need an extra export. Its start call must still go:
    // that start is libtest's `main`, which aborts the module before a case.
    let js_file = integration_dir.join(format!("{}.js", build.package_name));
    wasm::patch_bindgen_glue(&js_file, false)?;

    init::write_template_to(
        "bindgen-web/index.html",
        &build.package_name,
        integration_dir,
    )?;

    let bg_wasm = integration_dir.join(format!("{}_bg.wasm", build.package_name));
    let mut tests = wasm_test::discover_tests(&bg_wasm)?;
    if let Some(filter) = filter {
        tests.retain(|name| name.contains(filter));
    }
    if tests.is_empty() {
        return Err(WasmTestbedError::NoFwtCases(bg_wasm.display().to_string()).trace());
    }
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

    let output = browser::run(
        &url,
        browser,
        headless,
        &foundation_context::Ctx::host(),
        &browser::BrowserRunLimits::default(),
    )?;
    tracing::info!("Test output: {}", output.output);

    server.shutdown();
    // The page cannot set an exit code: the last `test result:` line is the
    // verdict. A page that printed none is not a pass.
    let summary = output.test_result.trim().to_string();
    if !summary.contains("test result:") {
        return Err(WasmTestbedError::MissingTestSummary(summary).trace());
    }
    if !summary.starts_with("test result: ok") {
        let failed = failed_count(&summary);
        // The whole page text names the failing cases (issue #588).
        return Err(WasmTestbedError::CaseFailures {
            failed,
            output: output.output,
        }
        .trace());
    }
    Ok(())
}

fn run_bindgen_deno(
    build: &build::BuildOutput,
    integration_dir: &std::path::Path,
    filter: Option<&str>,
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

    // The deno glue keeps the instance private and starts the module at
    // import time. Export `wasm` so run.js can call the `__wbgt_` shims, and
    // drop the libtest `main` start call (see `wasm::patch_bindgen_glue`).
    let js_file = integration_dir.join(format!("{}.js", build.package_name));
    wasm::patch_bindgen_glue(&js_file, true)?;

    let bg_wasm = integration_dir.join(format!("{}_bg.wasm", build.package_name));
    let mut tests = wasm_test::discover_tests(&bg_wasm)?;
    if let Some(filter) = filter {
        tests.retain(|name| name.contains(filter));
    }
    tracing::info!("Discovered {} tests", tests.len());

    init::write_bindgen_runjs_to(
        "bindgen-deno/run.js",
        &build.package_name,
        &tests,
        integration_dir,
    )?;

    // The suite exits non-zero when a case fails. Read the summary, and fail
    // when it and the exit code disagree.
    let output = deno::run_status(integration_dir, "run.js")?;
    if !output.stdout.is_empty() {
        println!("{}", output.stdout);
    }
    let Some(summary) = output
        .stdout
        .lines()
        .rfind(|line| line.contains("test result:"))
        .map(|line| line.trim().to_string())
    else {
        return Err(WasmTestbedError::MissingTestSummary(output.stdout).trace());
    };
    let passed = summary.starts_with("test result: ok");
    if passed != (output.exit_code == 0) {
        return Err(WasmTestbedError::VerdictMismatch {
            exit_code: output.exit_code,
            summary: summary.clone(),
        }
        .trace());
    }
    if !passed {
        let failed = failed_count(&summary);
        return Err(WasmTestbedError::CaseFailures {
            failed,
            output: output.stdout,
        }
        .trace());
    }
    Ok(())
}

/// The count before `failed` in a `test result:` summary line.
///
/// The line is `test result: FAILED. 1 passed; 1 failed; 0 ignored; …`.
fn failed_count(summary: &str) -> usize {
    summary
        .split(';')
        .find(|part| part.contains("failed"))
        .and_then(|part| part.split_whitespace().next())
        .and_then(|count| count.parse().ok())
        .unwrap_or(0)
}

fn run_bindgen_wrangler(
    build: &build::BuildOutput,
    integration_dir: &std::path::Path,
    filter: Option<&str>,
) -> Result<(), ErrorTrace<WasmTestbedError>> {
    use error::{ToTrace, WasmTestbedError};
    use wasm::BindgenTarget;

    tracing::info!("Running wasm-bindgen (module)...");
    wasm::run_wasm_bindgen_with_name(
        &build.wasm_path,
        integration_dir,
        BindgenTarget::EsModules,
        Some(&build.package_name),
    )?;

    // The `module` target instantiates the wasm at import time and keeps the
    // instance private. Export it as `wasm` so the worker can call the
    // `__wbgt_` test shims (raw wasm exports). Drop the libtest `main` start
    // call (see `wasm::patch_bindgen_glue`).
    let js_file = integration_dir.join(format!("{}.js", build.package_name));
    wasm::patch_bindgen_glue(&js_file, true)?;

    let bg_wasm = integration_dir.join(format!("{}_bg.wasm", build.package_name));
    let mut tests = wasm_test::discover_tests(&bg_wasm)?;
    if let Some(filter) = filter {
        tests.retain(|name| name.contains(filter));
    }
    if tests.is_empty() {
        return Err(WasmTestbedError::NoFwtCases(bg_wasm.display().to_string()).trace());
    }
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

    let output = wrangler::run(integration_dir, None, &foundation_context::Ctx::host())?;
    let passed = serde_json::from_str::<serde_json::Value>(&output.response_body)
        .ok()
        .and_then(|value| value.get("passed").and_then(serde_json::Value::as_bool));
    if output.status_code >= 400 || passed != Some(true) {
        return Err(WasmTestbedError::WranglerHttpFailed(format!(
            "status {}: {} (wrangler: {})",
            output.status_code, output.response_body, output.wrangler_stderr
        ))
        .trace());
    }
    println!("{}", output.response_body);
    Ok(())
}
