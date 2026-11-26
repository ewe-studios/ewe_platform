// Hosted-prelude import: this module is target-gated (native-only) inside a no_std crate.
#[allow(unused_imports)]
use std::prelude::rust_2021::*;
use std::println;

use std::path::PathBuf;

use clap::{ArgMatches, Command};

use crate::build_tools::WasmBinGenerator;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[must_use]
pub fn command() -> Command {
    Command::new("wasm_bins")
        .about("WASM binary entrypoint management")
        .arg_required_else_help(true)
        .subcommand(
            Command::new("list")
                .about("Dry-run: scan crate and list discovered WASM entrypoints")
                .arg(
                    clap::Arg::new("crate_directory")
                        .required(true)
                        .value_parser(clap::value_parser!(PathBuf))
                        .help("Path to the crate directory to scan"),
                ),
        )
        .subcommand(
            Command::new("generate")
                .about("Scan crate and generate WASM binary entrypoint files")
                .arg(
                    clap::Arg::new("crate_directory")
                        .required(true)
                        .value_parser(clap::value_parser!(PathBuf))
                        .help("Path to the crate directory to scan"),
                ),
        )
}

/// # Errors
///
/// Returns errors from `WasmBinGenerator` or I/O operations.
pub fn run(args: &ArgMatches) -> Result<(), BoxedError> {
    args.subcommand()
        .map_or_else(|| Ok(()), |(name, matches)| run_subcommand(name, matches))
}

/// Execute a parsed leaf command.
///
/// # Errors
/// Returns an error from the selected command.
pub fn run_subcommand(name: &str, arguments: &ArgMatches) -> Result<(), BoxedError> {
    match name {
        "list" => run_list(arguments),
        "generate" => run_generate(arguments),
        _ => Ok(()),
    }
}

fn run_list(args: &ArgMatches) -> Result<(), BoxedError> {
    let crate_path = args
        .get_one::<PathBuf>("crate_directory")
        .expect("crate_directory is required");

    let generator = WasmBinGenerator::new(crate_path)?;
    let plan = generator.plan()?;

    println!(
        "Scanning crate: {} ({})",
        plan.crate_name,
        plan.crate_dir.display()
    );
    println!();
    println!("Found {} WASM entrypoints:", plan.entrypoints.len());
    println!();

    for (i, ep) in plan.entrypoints.iter().enumerate() {
        println!("  {}. {}", i + 1, ep.name);
        println!("     Description: {}", ep.description);
        println!(
            "     Source: {}:{} ({})",
            ep.source_file.display(),
            ep.line,
            ep.qualified_path
        );
        println!("     Binary path: bin/{}/main.rs", ep.name);
        println!("     WASM output:");
    }

    for output in &plan.wasm_outputs {
        println!("       debug:   {}", output.debug_path);
        println!("       release: {}", output.release_path);
    }

    println!();
    println!("Cargo.toml changes (not applied):");
    for bin in &plan.bin_sections {
        println!(
            "  + [[bin]] name = \"{}\", path = \"{}\"",
            bin.name, bin.path
        );
    }
    println!();
    println!("No files were modified (dry run).");

    Ok(())
}

fn run_generate(args: &ArgMatches) -> Result<(), BoxedError> {
    let crate_path = args
        .get_one::<PathBuf>("crate_directory")
        .expect("crate_directory is required");

    let generator = WasmBinGenerator::new(crate_path)?;

    tracing::info!(
        "scanning crate {} ({})",
        generator.crate_name(),
        crate_path.display()
    );

    let plan = generator.generate()?;

    tracing::info!("generated {} WASM entrypoints", plan.entrypoints.len());
    for file in &plan.generated_files {
        tracing::info!("created {}", file.path.display());
    }
    tracing::info!(
        "updated Cargo.toml ({} [[bin]] sections added)",
        plan.bin_sections.len()
    );
    tracing::info!("build with: cargo build --target wasm32-unknown-unknown [--release]");

    Ok(())
}
