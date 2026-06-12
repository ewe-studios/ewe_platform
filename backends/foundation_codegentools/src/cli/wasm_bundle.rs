//! WHY: One command from annotation to deployable output (feature 10 §3):
//! `ewe-wasm build` orchestrates discovery → WASM compile → JS wrappers →
//! bundles → runtime assets.
//!
//! WHAT: The `wasm-bundle` subcommand (`build` + `plan`), with the spec's
//! flags: `--release`/`--dev` (conflicting), `--output`, `--target`,
//! `--skip-runtimes`, `--verbose`.
//!
//! HOW: Thin orchestration over [`crate::wasm_bundle::WasmBundleGenerator`].

use std::path::{Path, PathBuf};

use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::wasm_bundle::WasmBundleGenerator;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[must_use]
pub fn command() -> Command {
    Command::new("wasm-bundle")
        .about("Build WASM entrypoints with their JS wrappers and bundles (feature 10)")
        .subcommand_required(true)
        .subcommand(
            Command::new("build")
                .about("Compile + generate wrappers/bundles into the output directory")
                .arg(crate_dir_arg())
                .arg(
                    Arg::new("release")
                        .long("release")
                        .action(ArgAction::SetTrue)
                        .conflicts_with("dev")
                        .help("Compile WASM with optimizations"),
                )
                .arg(
                    Arg::new("dev")
                        .long("dev")
                        .action(ArgAction::SetTrue)
                        .help("Compile WASM in debug mode (default; uses the uat profile — wasm32 needs LLVM)"),
                )
                .arg(
                    Arg::new("output")
                        .long("output")
                        .default_value("build")
                        .help("Output directory"),
                )
                .arg(
                    Arg::new("skip_runtimes")
                        .long("skip-runtimes")
                        .action(ArgAction::SetTrue)
                        .help("Do not copy the JS runtime assets"),
                )
                .arg(
                    Arg::new("verbose")
                        .long("verbose")
                        .action(ArgAction::SetTrue)
                        .help("Print each build step"),
                ),
        )
        .subcommand(
            Command::new("plan")
                .about("Dry-run: list what build would generate")
                .arg(crate_dir_arg())
                .arg(Arg::new("output").long("output").default_value("build")),
        )
}

fn crate_dir_arg() -> Arg {
    Arg::new("crate_directory")
        .long("target")
        .default_value(".")
        .help("Crate directory to scan (the spec's --target <CRATE>)")
}

/// Run a parsed `wasm-bundle` invocation.
///
/// # Errors
/// Propagates generator failures (scan, compile, IO).
pub fn run(args: &ArgMatches) -> Result<(), BoxedError> {
    match args.subcommand() {
        Some(("build", sub)) => run_build(sub),
        Some(("plan", sub)) => run_plan(sub),
        _ => unreachable!("subcommand_required"),
    }
}

fn generator(args: &ArgMatches) -> Result<WasmBundleGenerator, BoxedError> {
    let crate_dir = args
        .get_one::<String>("crate_directory")
        .expect("defaulted");
    let output: PathBuf = args.get_one::<String>("output").expect("defaulted").into();
    Ok(WasmBundleGenerator::new(Path::new(crate_dir), &output)?)
}

fn run_plan(args: &ArgMatches) -> Result<(), BoxedError> {
    let generator = generator(args)?;
    println!("Discovered {} entrypoint(s):", generator.entrypoints().len());
    for ep in generator.entrypoints() {
        println!("  {} ({:?}, {:?})", ep.name, ep.mode, ep.packaging);
    }
    println!();
    println!("Would generate:");
    for file in generator.plan() {
        println!("  {} ({})", file.path.display(), file.kind);
    }
    Ok(())
}

fn run_build(args: &ArgMatches) -> Result<(), BoxedError> {
    let generator = generator(args)?;
    let release = args.get_flag("release");
    let skip_runtimes = args.get_flag("skip_runtimes");
    let verbose = args.get_flag("verbose");

    if verbose {
        println!(
            "Building {} entrypoint(s), release={release}, skip_runtimes={skip_runtimes}",
            generator.entrypoints().len()
        );
    }
    let written = generator.execute(release, skip_runtimes, &runtime_assets())?;
    for file in &written {
        println!("  Wrote: {} ({})", file.path.display(), file.kind);
    }
    println!("Done — {} file(s).", written.len());
    Ok(())
}

/// The owned runtime assets, resolved relative to the workspace layout.
fn runtime_assets() -> Vec<(&'static str, &'static Path)> {
    vec![
        (
            "foundation-wasm.js",
            Path::new("backends/foundation_wasm/runtime/foundation-wasm.js"),
        ),
        (
            "foundation-wasm-ui.js",
            Path::new("backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js"),
        ),
    ]
}
