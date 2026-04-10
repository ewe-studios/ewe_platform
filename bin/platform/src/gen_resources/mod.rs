//! WHY: Unified generator for resource types, API clients, provider specs, and model descriptors.
//!
//! WHAT: Combines gen_resource_types, gen_provider_clients, gen_provider_specs, and gen_model_descriptors
//! functionality with subcommands: `types`, `clients`, `specs`, and `models`.
//!
//! HOW: Delegates to respective generator modules based on subcommand.

mod clients;
mod model_descriptors;
mod provider_wrappers;
mod provider_specs;
mod provider_specs_core;
mod provider_specs_errors;
mod provider_specs_fetcher;
mod types;

use std::path::PathBuf;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

// ---------------------------------------------------------------------------
// CLI registration
// ---------------------------------------------------------------------------

/// Register the `gen_resources` subcommand with all generator sub-subcommands.
pub fn register(cmd: clap::Command) -> clap::Command {
    cmd.subcommand(
        clap::Command::new("gen_resources")
            .about("Generate Rust types, API clients, provider specs, and model descriptors")
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
            .subcommand(
                clap::Command::new("specs")
                    .about("Fetch and distill OpenAPI specs from deployment providers")
                    .arg(
                        clap::Arg::new("provider")
                            .long("provider")
                            .short('p')
                            .help("Fetch only this provider's spec (default: all)")
                            .value_name("PROVIDER"),
                    )
                    .arg(
                        clap::Arg::new("gcp-apis")
                            .long("gcp-apis")
                            .help("Comma-separated list of GCP API names to fetch (default: all APIs)")
                            .value_name("APIS"),
                    )
                    .arg(
                        clap::Arg::new("dry-run")
                            .long("dry-run")
                            .help("Fetch specs but don't write to disk")
                            .action(clap::ArgAction::SetTrue),
                    )
                    .arg(
                        clap::Arg::new("force")
                            .long("force")
                            .help("Write specs even if content hasn't changed")
                            .action(clap::ArgAction::SetTrue),
                    )
                    .arg(
                        clap::Arg::new("debug")
                            .long("debug")
                            .default_value("false")
                            .action(clap::ArgAction::SetTrue)
                            .help("Enables debug logs (default: false)"),
                    ),
            )
            .subcommand(
                clap::Command::new("models")
                    .about("Fetch upstream model catalogs and regenerate model descriptors")
                    .arg(
                        clap::Arg::new("debug")
                            .long("debug")
                            .default_value("false")
                            .action(clap::ArgAction::SetTrue)
                            .help("Enables debug logs (default: false)"),
                    ),
            )
            .subcommand(
                clap::Command::new("providers")
                    .about("Generate provider wrapper APIs with automatic state tracking")
                    .arg(
                        clap::Arg::new("provider")
                            .long("provider")
                            .short('p')
                            .help("Generate wrappers for only this provider (default: all). Use fly_io for Fly.io.")
                            .value_name("PROVIDER"),
                    )
                    .arg(
                        clap::Arg::new("output-dir")
                            .long("output-dir")
                            .help("Output directory for generated files")
                            .value_name("DIR")
                            .default_value("backends/foundation_deployment/src/providers"),
                    ),
            ),
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
                // Provider name uses underscores everywhere (fly_io, prisma_postgres, etc.)
                generator.generate_for_provider(provider)?;
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
                // Provider name uses underscores everywhere (fly_io, prisma_postgres, etc.)
                generator.generate_for_provider(provider)?;
            } else {
                generator.generate_all()?;
            }

            Ok(())
        }
        Some(("specs", sub_matches)) => {
            provider_specs::run(sub_matches)?;
            Ok(())
        }
        Some(("models", sub_matches)) => {
            model_descriptors::run(sub_matches)?;
            Ok(())
        }
        Some(("providers", sub_matches)) => {
            let provider = sub_matches.get_one::<String>("provider").map(|s| s.as_str());
            let output_dir = sub_matches
                .get_one::<String>("output-dir")
                .map(|s| PathBuf::from(s))
                .unwrap_or_else(|| PathBuf::from("backends/foundation_deployment/src/providers"));

            let artefacts_dir = PathBuf::from("artefacts/cloud_providers");

            let generator = provider_wrappers::ProviderWrapperGenerator::new(artefacts_dir, output_dir);

            if let Some(provider) = provider {
                generator.generate_for_provider(provider)?;
            } else {
                generator.generate_all()?;
            }

            Ok(())
        }
        _ => unreachable!("subcommand_required ensures a subcommand is present"),
    }
}
