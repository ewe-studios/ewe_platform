use std::path::Path;

use system_operations::wasm_bins::WasmBinGenerator;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// WHY: The platform binary needs a consistent registration pattern
/// for all subcommands.
///
/// WHAT: Registers the `wasm_bins` subcommand with `list` and `generate`
/// nested subcommands.
///
/// HOW: Uses clap's builder API to define the command tree.
///
/// # Panics
///
/// Never panics.
pub fn register(command: clap::Command) -> clap::Command {
    command.subcommand(
        clap::Command::new("wasm_bins")
            .about("WASM binary entrypoint management")
            .arg_required_else_help(true)
            .subcommand(
                clap::Command::new("list")
                    .about("Dry-run: scan crate and list discovered WASM entrypoints")
                    .arg(
                        clap::Arg::new("crate_directory")
                            .required(true)
                            .help("Path to the crate directory to scan"),
                    ),
            )
            .subcommand(
                clap::Command::new("generate")
                    .about("Scan crate and generate WASM binary entrypoint files")
                    .arg(
                        clap::Arg::new("crate_directory")
                            .required(true)
                            .help("Path to the crate directory to scan"),
                    ),
            ),
    )
}

/// WHY: The platform binary dispatches to subcommand handlers via match.
///
/// WHAT: Handles `wasm_bins` subcommand execution by delegating to
/// `list` or `generate` handlers.
///
/// HOW: Matches on the subcommand name and calls the appropriate handler.
///
/// # Errors
///
/// Returns errors from `WasmBinGenerator` or I/O operations.
pub fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    match args.subcommand() {
        Some(("list", sub_args)) => run_list(sub_args),
        Some(("generate", sub_args)) => run_generate(sub_args),
        _ => Ok(()),
    }
}

fn run_list(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let crate_dir = args
        .get_one::<String>("crate_directory")
        .expect("crate_directory is required");
    let crate_path = Path::new(crate_dir);

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

    // Find matching wasm outputs
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

fn run_generate(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let crate_dir = args
        .get_one::<String>("crate_directory")
        .expect("crate_directory is required");
    let crate_path = Path::new(crate_dir);

    let generator = WasmBinGenerator::new(crate_path)?;

    println!(
        "Scanning crate: {} ({})",
        generator.crate_name(),
        crate_path.display()
    );

    let plan = generator.generate()?;

    println!();
    println!("Generated {} WASM entrypoints:", plan.entrypoints.len());
    println!();

    for file in &plan.generated_files {
        println!("  Created: {}", file.path.display());
    }
    println!(
        "  Updated: Cargo.toml (added {} [[bin]] sections)",
        plan.bin_sections.len()
    );

    println!();
    println!("Build with:");
    println!("  cargo build --target wasm32-unknown-unknown");
    println!("  cargo build --target wasm32-unknown-unknown --release");

    Ok(())
}
