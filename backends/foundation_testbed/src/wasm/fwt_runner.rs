//! WHY: Feature 12 — contributors run `wasm-testbed node <crate>` instead of
//! hand-rolling `cargo build --target wasm32 … && node --test`. The full loop is
//! owned: build → discover → stage → run → report, zero wasm-bindgen/wasm-pack.
//!
//! WHAT: [`run_node`], [`run_deno`], [`run_web`] — build a `foundation_wasm` cdylib
//! to wasm32 (LLVM backend), discover `__fwt_` cases, stage a self-contained
//! harness (embedded `foundation-wasm.js` runtime + generic `runner.mjs` + the
//! module + `cases.json`), execute it on the chosen host, and surface the exit
//! code/summary.
//!
//! HOW: Staging goes to a temp dir (everything in it is generated; nothing to
//! commit). The SAME runner script serves all three hosts: node runs it directly,
//! deno via `deno run -A`, the browser via an `index.html` shell that Playwright
//! polls for the `test result:` summary (reusing the existing server/browser
//! plumbing). The runtime assets come from `foundation_wasm_ui`'s `embedded-js`
//! feature, so the staged harness needs no repo paths.

use std::path::{Path, PathBuf};
use std::process::Command;

use tracing::{debug, info};

use crate::wasm::cli::OwnedRunArgs;
use crate::wasm::error::{Result, ToTrace, WasmTestbedError};
use crate::wasm::fwt::FwtCase;
use crate::wasm::init::read_template;
use crate::wasm::{build, fwt, server};

/// Verify every tool a runner needs is on PATH BEFORE doing any work, so a
/// missing prerequisite is one clear actionable error rather than a mid-run
/// failure after a multi-minute wasm build.
fn preflight(tools: &[(&str, &str)]) -> Result<()> {
    for (tool, why) in tools {
        if which::which(tool).is_err() {
            return Err(WasmTestbedError::MissingTool {
                tool: (*tool).to_string(),
                why: (*why).to_string(),
            }
            .trace());
        }
    }
    Ok(())
}

/// Outcome of an owned-harness run.
#[derive(Debug)]
pub struct RunOutcome {
    /// Per-case + summary lines the runner printed.
    pub output: String,
    /// Process (or browser-summary) exit code: 0 = all green.
    pub exit_code: i32,
    /// The discovered cases that were staged.
    pub cases: Vec<FwtCase>,
}

/// Build + discover + stage into `stage_dir` → path of the staged runner script.
///
/// # Errors
/// Fails when the build, discovery, template read, or any staging write fails.
pub fn stage(crate_path: &Path, args: &OwnedRunArgs, stage_dir: &Path) -> Result<Vec<FwtCase>> {
    let built = build::run(crate_path, args.release, args.features.as_deref())?;
    let mut cases = fwt::discover_cases(&built.wasm_path)?;
    if let Some(filter) = &args.filter {
        cases.retain(|case| case.name.contains(filter.as_str()));
    }
    if cases.is_empty() {
        return Err(WasmTestbedError::NoFwtCases(built.wasm_path.display().to_string()).trace());
    }

    std::fs::create_dir_all(stage_dir).map_err(|e| WasmTestbedError::Io(e).trace())?;
    let write = |name: &str, bytes: &[u8]| -> Result<()> {
        std::fs::write(stage_dir.join(name), bytes).map_err(|e| WasmTestbedError::Io(e).trace())
    };

    // The owned runtime, embedded — the staged harness is fully self-contained.
    write(
        "foundation-wasm.js",
        foundation_wasm_ui::embedded::FOUNDATION_WASM_JS.as_bytes(),
    )?;
    write(
        "foundation-wasm-ui.js",
        foundation_wasm_ui::embedded::FOUNDATION_WASM_UI_JS.as_bytes(),
    )?;
    write("runner.mjs", read_template("fwt/runner.mjs")?.as_bytes())?;
    write("index.html", read_template("fwt/index.html")?.as_bytes())?;
    write("cases.json", cases_json(&cases).as_bytes())?;
    let wasm_bytes =
        std::fs::read(&built.wasm_path).map_err(|e| WasmTestbedError::WasmReadFailed(e).trace())?;
    write("module.wasm", &wasm_bytes)?;

    info!(
        "staged {} case(s) from {} into {}",
        cases.len(),
        built.package_name,
        stage_dir.display()
    );
    Ok(cases)
}

fn cases_json(cases: &[FwtCase]) -> String {
    // Hand-rolled to keep the schema explicit (it is the runner's contract).
    let mut out = String::from("[");
    for (index, case) in cases.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"name\":{name:?},\"export\":{export:?},\"is_async\":{is_async},\"should_panic\":{should_panic},\"ignore\":{ignore}}}",
            name = case.name,
            export = case.export,
            is_async = case.is_async,
            should_panic = case.should_panic,
            ignore = case.ignore,
        ));
    }
    out.push(']');
    out
}

fn staged_temp(crate_path: &Path, args: &OwnedRunArgs) -> Result<(tempfile::TempDir, Vec<FwtCase>)> {
    let dir = tempfile::TempDir::new().map_err(|e| WasmTestbedError::Io(e).trace())?;
    let cases = stage(crate_path, args, dir.path())?;
    Ok((dir, cases))
}

fn run_host(host: &str, host_args: &[&str], cwd: &Path) -> Result<(String, i32)> {
    which::which(host).map_err(|_| WasmTestbedError::HostRuntimeNotFound(host.to_string()).trace())?;
    let output = Command::new(host)
        .args(host_args)
        .current_dir(cwd)
        .output()
        .map_err(|e| WasmTestbedError::Io(e).trace())?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        debug!("{host} stderr: {stderr}");
    }
    print!("{stdout}");
    Ok((stdout, output.status.code().unwrap_or(1)))
}

/// `wasm-testbed node <crate>` — the default owned mode.
///
/// # Errors
/// Fails on build/discovery/staging errors or when node is unavailable.
pub fn run_node(args: &OwnedRunArgs) -> Result<RunOutcome> {
    preflight(&[
        ("cargo", "builds the test crate to wasm32"),
        ("node", "executes the staged runner (install from https://nodejs.org)"),
    ])?;
    let crate_path = canonical(&args.crate_path)?;
    let (dir, cases) = staged_temp(&crate_path, args)?;
    let (output, exit_code) = run_host("node", &["runner.mjs"], dir.path())?;
    Ok(RunOutcome {
        output,
        exit_code,
        cases,
    })
}

/// `wasm-testbed deno <crate>` — headless wasm under Deno, same runner script.
///
/// # Errors
/// Fails on build/discovery/staging errors or when deno is unavailable.
pub fn run_deno(args: &OwnedRunArgs) -> Result<RunOutcome> {
    preflight(&[
        ("cargo", "builds the test crate to wasm32"),
        ("deno", "executes the staged runner (install from https://deno.land)"),
    ])?;
    let crate_path = canonical(&args.crate_path)?;
    let (dir, cases) = staged_temp(&crate_path, args)?;
    let (output, exit_code) = run_host("deno", &["run", "-A", "runner.mjs"], dir.path())?;
    Ok(RunOutcome {
        output,
        exit_code,
        cases,
    })
}

/// `wasm-testbed web <crate>` — serve the staged harness and run it under
/// Playwright on OUR runtime (no bindgen glue).
///
/// # Errors
/// Fails on build/discovery/staging errors or when the browser run fails.
pub fn run_web(args: &OwnedRunArgs) -> Result<RunOutcome> {
    preflight(&[
        ("cargo", "builds the test crate to wasm32"),
        ("node", "runs the Playwright driver script"),
        ("npm", "installs Playwright for the driver (or run `mise run setup:playwright` once)"),
        ("npx", "launches the Playwright browser installer"),
    ])?;
    let crate_path = canonical(&args.crate_path)?;
    let (dir, cases) = staged_temp(&crate_path, args)?;

    let serving = server::start_serving(dir.path())?;
    let url = serving.url("index.html");
    info!("serving owned harness at {url}");
    let outcome = crate::wasm::browser::run(&url, &args.browser, args.headless);
    serving.shutdown();
    let browser_output = outcome?;

    // The summary line is the verdict (the browser cannot set an exit code).
    let exit_code = i32::from(!browser_output.test_result.contains("test result: ok"));
    println!("{}", browser_output.test_result);
    Ok(RunOutcome {
        output: browser_output.test_result,
        exit_code,
        cases,
    })
}

fn canonical(path: &PathBuf) -> Result<PathBuf> {
    std::fs::canonicalize(path)
        .map_err(|_| WasmTestbedError::CratePathNotFound(path.display().to_string()).trace())
}
