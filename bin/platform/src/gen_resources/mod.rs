//! WHY: Unified generator for resource types and API clients from OpenAPI specs.
//!
//! WHAT: Combines gen_resource_types and gen_provider_clients functionality with
//! subcommands: `types` and `clients`.
//!
//! HOW: Delegates to respective generator modules based on subcommand.

mod clients;
mod types;

use std::path::PathBuf;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Normalize provider name from CLI (fly_io -> fly-io) to match artefacts directory.
fn normalize_provider_name(provider: &str) -> String {
    provider.replace('_', "-")
}

// ---------------------------------------------------------------------------
// CLI registration
// ---------------------------------------------------------------------------

/// Register the `gen_resources` subcommand with `types` and `clients` sub-subcommands.
pub fn register(cmd: clap::Command) -> clap::Command {
    cmd.subcommand(
        clap::Command::new("gen_resources")
            .about("Generate Rust types and API clients from OpenAPI specs")
            .subcommand_required(true)
            .subcommand(
                clap::Command::new("types")
                    .about("Generate Rust resource types from OpenAPI specs")
                    .arg(
                        clap::Arg::new("provider")
                            .long("provider")
                            .short('p')
                            .help("Generate types for only this provider (default: all). Use fly_io for Fly.io.")
                            .value_name("PROVIDER"),
                    )
                    .arg(
                        clap::Arg::new("output-dir")
                            .long("output-dir")
                            .help("Output directory for generated files")
                            .value_name("DIR")
                            .default_value("backends/foundation_deployment/src/providers"),
                    ),
            )
            .subcommand(
                clap::Command::new("clients")
                    .about("Generate Rust API client functions from OpenAPI specs")
                    .arg(
                        clap::Arg::new("provider")
                            .long("provider")
                            .short('p')
                            .help("Generate clients for only this provider (default: all). Use fly_io for Fly.io.")
                            .value_name("PROVIDER"),
                    )
                    .arg(
                        clap::Arg::new("output-dir")
                            .long("output-dir")
                            .help("Output directory for generated files")
                            .value_name("DIR")
                            .default_value("backends/foundation_deployment/src/providers"),
                    ),
            )
    )
}

/// Run the `gen_resources` command.
pub fn run(matches: &clap::ArgMatches) -> Result<(), BoxedError> {
    match matches.subcommand() {
        Some(("types", sub_matches)) => {
            let provider = sub_matches.get_one::<String>("provider").map(|s| s.as_str());
            let output_dir = sub_matches
                .get_one::<String>("output-dir")
                .map(|s| PathBuf::from(s))
                .unwrap_or_else(|| PathBuf::from("backends/foundation_deployment/src/providers"));

            let artefacts_dir = PathBuf::from("artefacts/cloud_providers");

            let generator = types::ResourceGenerator::new(artefacts_dir, output_dir);

            if let Some(provider) = provider {
                // Normalize provider name (fly_io -> fly-io) for directory lookup
                let normalized = normalize_provider_name(provider);
                generator.generate_for_provider(&normalized)?;
            } else {
                generator.generate_all()?;
            }

            Ok(())
        }
        Some(("clients", sub_matches)) => {
            let provider = sub_matches.get_one::<String>("provider").map(|s| s.as_str());
            let output_dir = sub_matches
                .get_one::<String>("output-dir")
                .map(|s| PathBuf::from(s))
                .unwrap_or_else(|| PathBuf::from("backends/foundation_deployment/src/providers"));

            let artefacts_dir = PathBuf::from("artefacts/cloud_providers");

            let generator = clients::ClientGenerator::new(artefacts_dir, output_dir);

            if let Some(provider) = provider {
                // Normalize provider name (fly_io -> fly-io) for directory lookup
                let normalized = normalize_provider_name(provider);
                generator.generate_for_provider(&normalized)?;
            } else {
                generator.generate_all()?;
            }

            Ok(())
        }
        _ => unreachable!("subcommand_required ensures a subcommand is present"),
    }
}
