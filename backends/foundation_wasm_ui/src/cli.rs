//! WHY: One command from annotation to deployable output (feature 10 §3):
//! `ewe-wasm build` orchestrates discovery → WASM compile → JS wrappers →
//! bundles → runtime assets.
//!
//! WHAT: The `wasm-bundle` subcommand (`build` + `plan`), with the spec's
//! flags: `--release`/`--dev` (conflicting), `--output`, `--target`,
//! `--skip-runtimes`, `--verbose`.
//!
//! HOW: Thin orchestration over [`crate::wasm_bundle::WasmBundleGenerator`].

// Hosted-prelude import: this module is target-gated (native-only) inside a no_std crate.
#[allow(unused_imports)]
use std::prelude::rust_2021::*;

use std::path::{Path, PathBuf};

use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::build_tools::WasmBundleGenerator;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[must_use]
pub fn command() -> Command {
    let command = Command::new("wasm-bundle")
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
                        .value_parser(clap::value_parser!(PathBuf))
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
                .arg(Arg::new("output").long("output").default_value("build").value_parser(clap::value_parser!(PathBuf))),
        );
    #[cfg(feature = "tasks")]
    let command = command.subcommand(<crate::tasks::RuntimeArgs as clap::Args>::augment_args(
        Command::new("runtime").about("Generate or verify the classic browser runtime scripts"),
    ));
    #[cfg(feature = "tasks")]
    let command = command.subcommand(
        Command::new("bindings")
            .about("Create per-instance browser bindings from wasm-bindgen bundler output")
            .arg(
                Arg::new("input")
                    .required(true)
                    .value_parser(clap::value_parser!(PathBuf)),
            )
            .arg(
                Arg::new("output")
                    .required(true)
                    .value_parser(clap::value_parser!(PathBuf)),
            ),
    );
    command
}

fn crate_dir_arg() -> Arg {
    Arg::new("crate_directory")
        .long("target")
        .default_value(".")
        .value_parser(clap::value_parser!(PathBuf))
        .help("Crate directory to scan (the spec's --target <CRATE>)")
}

/// Run a parsed `wasm-bundle` invocation.
///
/// # Errors
/// Propagates generator failures (scan, compile, IO).
pub fn run(args: &ArgMatches) -> Result<(), BoxedError> {
    args.subcommand().map_or_else(
        || unreachable!("subcommand_required"),
        |(name, matches)| run_subcommand(name, matches),
    )
}

/// Execute a parsed leaf command.
///
/// # Errors
/// Returns an error from the selected command.
pub fn run_subcommand(name: &str, arguments: &ArgMatches) -> Result<(), BoxedError> {
    match name {
        #[cfg(feature = "tasks")]
        "runtime" => {
            <crate::tasks::RuntimeArgs as clap::FromArgMatches>::from_arg_matches(arguments)?
                .generate()?;
            Ok(())
        }
        #[cfg(feature = "tasks")]
        "bindings" => {
            let input = arguments
                .try_get_one::<PathBuf>("input")?
                .ok_or("Missing input binding file")?;
            let output = arguments
                .try_get_one::<PathBuf>("output")?
                .ok_or("Missing output binding file")?;
            bindings(input, output)?;
            Ok(())
        }
        "build" => run_build(arguments),
        "plan" => run_plan(arguments),
        _ => unreachable!("subcommand_required"),
    }
}

fn generator(args: &ArgMatches) -> Result<WasmBundleGenerator, BoxedError> {
    let crate_dir = args
        .get_one::<PathBuf>("crate_directory")
        .expect("defaulted");
    let output = args.get_one::<PathBuf>("output").expect("defaulted");
    Ok(WasmBundleGenerator::new(crate_dir, output)?)
}

fn run_plan(args: &ArgMatches) -> Result<(), BoxedError> {
    let generator = generator(args)?;
    println!(
        "Discovered {} entrypoint(s):",
        generator.entrypoints().len()
    );
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
        tracing::info!(
            "building {} entrypoint(s), release={release}, skip_runtimes={skip_runtimes}",
            generator.entrypoints().len()
        );
    }
    let written = generator.execute(release, skip_runtimes, &runtime_assets())?;
    for file in &written {
        tracing::info!("wrote {} ({})", file.path.display(), file.kind);
    }
    tracing::info!("done — {} file(s)", written.len());
    Ok(())
}

/// The owned runtime assets, resolved relative to the workspace layout.
fn runtime_assets() -> Vec<(&'static str, &'static Path)> {
    let mut assets = vec![(
        "foundation-wasm.js",
        Path::new("backends/foundation_wasm/runtime/foundation-wasm.js"),
    )];
    macro_rules! browser_assets {
        ($($name:literal),+ $(,)?) => {
            $(assets.push(($name, Path::new(concat!("backends/foundation_wasm_ui/runtimes/", $name))));)+
        };
    }
    browser_assets!(
        "foundation-wasm-ui.js",
        "foundation-ui-frames.js",
        "foundation-ui-live.js",
        "foundation-ui-recovery.js",
        "foundation-ui-connections.js",
        "foundation-ui-vocab.js",
        "foundation-ui-wire.js",
        "foundation-ui-behaviours.js",
        "foundation-ui-bootstrap.js",
        "foundation-ui-actions.js",
        "platform-scheme-interceptor.js",
    );
    assets.push((
        "bundle.js",
        Path::new("backends/foundation_wasm_ui/runtimes/foundation-ui-runtime.min.js"),
    ));
    assets
}

/// Give each host its own binding functions, memory views, and finalizers.
#[cfg(feature = "tasks")]
fn bindings(input: &Path, output: &Path) -> std::io::Result<()> {
    use std::{
        fs,
        io::{self, Write},
    };
    let leaf = input
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::other("Expected a generated JavaScript filename"))?;
    let dir = input
        .parent()
        .ok_or_else(|| io::Error::other("Missing binding directory"))?;
    let temp = tempfile::Builder::new()
        .prefix("bindings-")
        .suffix(".js")
        .tempfile_in(dir)?;
    let module = serde_json::to_string(&format!("./{leaf}"))?;
    fs::write(
        temp.path(),
        format!("import * as functions from {module}; receiveBindings(functions);"),
    )?;
    let bundle = foundation_codegen::javascript::bundle(temp.path())
        .map_err(|e| io::Error::other(format!("{e:#}")))?;
    let mut file = fs::File::create(output)?;
    writeln!(file, "export const importName = {module};")?;
    file.write_all(b"export function createBindings() {\nlet bindings;\nfunction receiveBindings(value) { bindings = value; }\n")?;
    file.write_all(&bundle.readable)?;
    file.write_all(b"\nreturn bindings;\n}\n")
}
