//! F52: `wasm-testbed browser <crate>` — end-to-end wasm-bindgen browser tests.
//!
//! WHY: The `test bindgen-web` sub-mode was buried under the `test` command and
//! required the user to know about `Mode::BindgenWeb`. This runner lifts that
//! logic into a top-level `browser` subcommand, matching the `deno`/`web`
//! command shape, for a single-command developer experience.
//!
//! WHAT: build → wasm-bindgen → discover tests → stage harness → serve →
//! Chromium → poll for results.
//!
//! HOW: Reuses the existing building blocks: [`build::run_with_tests`],
//! [`wasm::run_wasm_bindgen_with_name`], [`wasm_test::discover_tests`],
//! [`init::write_template_to`] / [`init::write_bindgen_runjs_to`],
//! [`server::start_serving`], and [`browser::run`] (the pure-Rust CDP driver).

use tracing::info;

use crate::wasm::cli::BindgenArgs;
use crate::wasm::error::{Result, ToTrace, WasmTestbedError};
use crate::wasm::{build, browser, init, server, wasm, wasm_test};

/// Outcome mirroring the owned-harness shape so the CLI dispatch is uniform.
#[derive(Debug)]
pub struct RunOutcome {
    pub output: String,
    pub exit_code: i32,
    pub test_count: usize,
}

/// Check the installed `wasm-bindgen` CLI version is >= the crate version.
///
/// The crate version is pinned by `foundation_testbed` (single source of truth).
/// A newer CLI can process older crate output, but an older CLI cannot process
/// a newer crate's output — so we allow CLI >= crate, not exact equality.
fn check_wasm_bindgen_version() -> Result<()> {
    let output = std::process::Command::new("wasm-bindgen")
        .arg("--version")
        .output()
        .map_err(|e| WasmTestbedError::WasmBindgenExecFailed(e).trace())?;

    if !output.status.success() {
        return Err(WasmTestbedError::WasmBindgenFailed(output.status.code()).trace());
    }

    let cli_version = String::from_utf8_lossy(&output.stdout)
        .trim()
        .strip_prefix("wasm-bindgen ")
        .unwrap_or("(unknown)")
        .to_string();

    let crate_version = crate::bindgen::WASM_BINDGEN_VERSION;

    // Parse major.minor.patch as integers for numeric comparison.
    let parse_semver = |v: &str| -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() < 3 {
            return None;
        }
        Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
    };

    let (Some(cli_ver), Some(crate_ver)) = (parse_semver(&cli_version), parse_semver(crate_version)) else {
        return Err(WasmTestbedError::WasmBindgenVersionMismatch {
            cli: cli_version,
            crate_version: crate_version.to_string(),
        }
        .trace());
    };

    if cli_ver < crate_ver {
        return Err(WasmTestbedError::WasmBindgenVersionMismatch {
            cli: cli_version,
            crate_version: crate_version.to_string(),
        }
        .trace());
    }

    info!("wasm-bindgen CLI {cli_version} (crate {crate_version})");
    Ok(())
}

/// `wasm-testbed browser <crate>` — build → wasm-bindgen → stage → Chromium.
///
/// # Errors
/// Fails on missing tools, build failure, bindgen failure, test discovery
/// failure, or browser timeout.
pub fn run_bindgen(args: &BindgenArgs) -> Result<RunOutcome> {
    check_wasm_bindgen_version()?;

    // Inline preflight — avoids depending on `fwt_runner::preflight` which
    // requires the `wasm` feature.
    for (tool, why) in &[("cargo", "builds the test crate to wasm32"), ("wasm-bindgen", "generates JS glue")] {
        if which::which(tool).is_err() {
            return Err(WasmTestbedError::MissingTool {
                tool: (*tool).to_string(),
                why: (*why).to_string(),
            }.trace());
        }
    }

    let crate_path = canonical(&args.crate_path)?;

    // 1. Build with --tests so #[wasm_bindgen_test] expansions are compiled.
    let built = build::run_with_tests(&crate_path, args.release, args.features.as_deref())?;
    info!(
        "built {} ({}) for wasm32-unknown-unknown",
        built.package_name, built.profile
    );

    // 2. Stage everything into a temp dir.
    let staging = tempfile::TempDir::new().map_err(|e| WasmTestbedError::Io(e).trace())?;
    let staging_path = staging.path().to_path_buf();

    // 2a. wasm-bindgen → {name}.js + {name}_bg.wasm
    wasm::run_wasm_bindgen_with_name(
        &built.wasm_path,
        &staging_path,
        wasm::BindgenTarget::Web,
        Some(&built.package_name),
    )?;

    // 2b. index.html (the page the browser loads)
    init::write_template_to(
        "bindgen-web/index.html",
        &built.package_name,
        &staging_path,
    )?;

    // 2c. Discover tests from the bindgen-processed wasm
    let bg_wasm = staging_path.join(format!("{}_bg.wasm", built.package_name));
    let mut tests = wasm_test::discover_tests(&bg_wasm)?;
    if let Some(filter) = &args.filter {
        tests.retain(|name| name.contains(filter.as_str()));
    }
    if tests.is_empty() {
        return Err(
            WasmTestbedError::NoFwtCases(bg_wasm.display().to_string()).trace(),
        );
    }
    info!("discovered {} browser test(s)", tests.len());

    // 2d. run.js — imports the bindgen glue + runs the discovered tests
    init::write_bindgen_runjs_to(
        "bindgen-web/run.js",
        &built.package_name,
        &tests,
        &staging_path,
    )?;

    // 3. Serve + Chromium
    let serving = server::start_serving(&staging_path)?;
    let url = serving.url("index.html");
    info!("serving at {url}");

    let output = browser::run(&url, &crate::wasm::cli::Browser::Chrome, args.headless)?;
    serving.shutdown();

    let exit_code = i32::from(!output.test_result.contains("test result: ok"));
    info!("browser test result: {}", output.test_result);

    Ok(RunOutcome {
        output: output.test_result,
        exit_code,
        test_count: tests.len(),
    })
}

fn canonical(path: &std::path::PathBuf) -> Result<std::path::PathBuf> {
    std::fs::canonicalize(path)
        .map_err(|_| WasmTestbedError::CratePathNotFound(path.display().to_string()).trace())
}
