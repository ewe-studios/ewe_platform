//! WHY: Unified generator command that combines types, clients, and providers generation.
//!
//! WHAT: Single command that analyzes OpenAPI specs and generates all artifacts together.
//!
//! HOW: Uses foundation_openapi for analysis and generation with intelligent grouping.

use foundation_openapi::{
    UnifiedGenerator,
    unified::{analyze_spec, AnalysisOptions},
};
use std::path::PathBuf;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

// ---------------------------------------------------------------------------
// CLI registration
// ---------------------------------------------------------------------------

/// Register the `gen_api` subcommand.
pub fn register(cmd: clap::Command) -> clap::Command {
    cmd.subcommand(
        clap::Command::new("gen_api")
            .about("Unified generator - generates types, clients, and providers from OpenAPI specs")
            .subcommand_required(true)
            .arg_required_else_help(true)
            .subcommand(
                clap::Command::new("generate")
                    .about("Generate all artifacts (types, clients, providers) from OpenAPI specs")
                    .arg(
                        clap::Arg::new("provider")
                            .help("Provider name (e.g., gcp, cloudflare, stripe)")
                            .required(true)
                            .value_name("PROVIDER"),
                    )
                    .arg(
                        clap::Arg::new("output-dir")
                            .long("output-dir")
                            .short('o')
                            .help("Output directory for generated files")
                            .value_name("DIR")
                            .default_value("backends/foundation_deployment/src/providers"),
                    )
                    .arg(
                        clap::Arg::new("dry-run")
                            .long("dry-run")
                            .short('n')
                            .help("Analyze and show grouping without writing files")
                            .action(clap::ArgAction::SetTrue),
                    )
                    .arg(
                        clap::Arg::new("features")
                            .long("features")
                            .help("Generate with per-group feature flags")
                            .action(clap::ArgAction::SetTrue),
                    )
                    .arg(
                        clap::Arg::new("min-group-size")
                            .long("min-group-size")
                            .help("Minimum endpoints per group")
                            .value_name("N")
                            .default_value("10"),
                    )
                    .arg(
                        clap::Arg::new("max-group-size")
                            .long("max-group-size")
                            .help("Maximum endpoints per group")
                            .value_name("N")
                            .default_value("200"),
                    ),
            )
            .subcommand(
                clap::Command::new("analyze")
                    .about("Analyze OpenAPI spec and show grouping without generating")
                    .arg(
                        clap::Arg::new("provider")
                            .help("Provider name (e.g., gcp, cloudflare, stripe)")
                            .required(true)
                            .value_name("PROVIDER"),
                    )
                    .arg(
                        clap::Arg::new("spec")
                            .long("spec")
                            .short('s')
                            .help("Path to OpenAPI spec JSON file")
                            .required(true)
                            .value_name("PATH"),
                    )
                    .arg(
                        clap::Arg::new("min-group-size")
                            .long("min-group-size")
                            .help("Minimum endpoints per group")
                            .value_name("N")
                            .default_value("10"),
                    )
                    .arg(
                        clap::Arg::new("max-group-size")
                            .long("max-group-size")
                            .help("Maximum endpoints per group")
                            .value_name("N")
                            .default_value("200"),
                    ),
            ),
    )
}

/// Run the `gen_api` command.
pub fn run(matches: &clap::ArgMatches) -> Result<(), BoxedError> {
    match matches.subcommand() {
        Some(("analyze", sub_matches)) => {
            let provider = sub_matches.get_one::<String>("provider").unwrap();
            let spec_path = sub_matches.get_one::<String>("spec").unwrap();
            let min_group_size = sub_matches
                .get_one::<String>("min-group-size")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(10);
            let max_group_size = sub_matches
                .get_one::<String>("max-group-size")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(200);

            // Load spec
            let spec_content = std::fs::read_to_string(spec_path)
                .map_err(|e| format!("Failed to read spec at {}: {}", spec_path, e))?;

            // Analyze
            let options = AnalysisOptions {
                min_group_size,
                max_group_size,
            };

            let analysis = analyze_spec(&spec_content, provider, &options)
                .map_err(|e| format!("Analysis failed: {}", e))?;

            // Print analysis results
            println!("\n=== OpenAPI Spec Analysis for '{}' ===", provider);
            println!("Total groups: {}", analysis.groups.len());
            println!("Shared resources: {}", analysis.shared_resources.len());

            for (i, group) in analysis.groups.iter().enumerate() {
                println!("\n  Group {}: {}", i + 1, group.name);
                println!("    Endpoints: {}", group.endpoints.len());
                println!("    Response types: {}", group.response_types.len());
            }

            if !analysis.shared_resources.is_empty() {
                println!("\n  Shared Resources ({} types):", analysis.shared_resources.len());
                for resource in &analysis.shared_resources {
                    println!("    - {}", resource);
                }
            }

            println!("\n=== Analysis Complete ===");

            Ok(())
        }
        Some(("generate", sub_matches)) => {
            let provider = sub_matches.get_one::<String>("provider").unwrap();
            let output_dir = sub_matches
                .get_one::<String>("output-dir")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("backends/foundation_deployment/src/providers"));
            let dry_run = sub_matches.get_flag("dry-run");
            let with_features = sub_matches.get_flag("features");
            let min_group_size = sub_matches
                .get_one::<String>("min-group-size")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(10);
            let max_group_size = sub_matches
                .get_one::<String>("max-group-size")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(200);

            let artefacts_dir = PathBuf::from("artefacts/cloud_providers");

            // Load spec from artefacts directory
            // Try openapi.json first (cloudflare, fly_io, etc.), then {provider}.json
            let provider_dir = artefacts_dir.join(provider);
            let spec_path = if provider_dir.join("openapi.json").exists() {
                provider_dir.join("openapi.json")
            } else if provider_dir.join(format!("{}.json", provider)).exists() {
                provider_dir.join(format!("{}.json", provider))
            } else {
                // For GCP-like structure with multiple APIs, use first available
                if let Ok(mut entries) = std::fs::read_dir(&provider_dir) {
                    if let Some(entry) = entries.next() {
                        if let Ok(entry) = entry {
                            let api_dir = entry.path();
                            if api_dir.is_dir() {
                                api_dir.join("openapi.json")
                            } else {
                                return Err(format!("No valid OpenAPI spec found for provider '{}'", provider).into());
                            }
                        } else {
                            return Err(format!("Cannot read provider directory for '{}'", provider).into());
                        }
                    } else {
                        return Err(format!("Provider '{}' has no API specs", provider).into());
                    }
                } else {
                    return Err(format!("Provider '{}' not found in artefacts", provider).into());
                }
            };

            let options = AnalysisOptions {
                min_group_size,
                max_group_size,
            };

            // Load and analyze spec
            let spec_content = std::fs::read_to_string(&spec_path)
                .map_err(|e| format!("Failed to read spec at {}: {}", spec_path.display(), e))?;

            let _analysis = analyze_spec(&spec_content, provider, &options)
                .map_err(|e| format!("Analysis failed: {}", e))?;

            if dry_run {
                println!("\n=== Dry Run Mode ===");
                println!("Provider: {}", provider);
                println!("Spec: {}", spec_path.display());
                println!("Output: {}", output_dir.display());
                println!("Features: {}", if with_features { "enabled" } else { "disabled" });
                println!("\nAnalysis complete - no files written.");
                return Ok(());
            }

            // Generate using unified generator
            println!("\n=== Generating for '{}' ===", provider);

            let generator = UnifiedGenerator::new(output_dir.clone());
            generator.generate(provider, &spec_content, &options)?;

            println!("\n=== Generation Complete ===");
            println!("Output directory: {}", output_dir.display());

            // Auto-fix common issues (unused imports, snake_case, etc.)
            println!("\n=== Running cargo fix ===");
            let feature_name = provider.replace('-', "_");
            let fix_output = std::process::Command::new("cargo")
                .args([
                    "fix",
                    "--lib",
                    "-p",
                    "foundation_deployment",
                    "--allow-dirty",
                    "--features",
                    &feature_name,
                ])
                .output()
                .expect("cargo fix should run");

            if !fix_output.stdout.is_empty() {
                println!("{}", String::from_utf8_lossy(&fix_output.stdout));
            }
            if !fix_output.stderr.is_empty() {
                eprintln!("{}", String::from_utf8_lossy(&fix_output.stderr));
            }

            Ok(())
        }
        _ => unreachable!("subcommand_required ensures a subcommand is present"),
    }
}
