//! WHY: Unified generator command that combines types, clients, and providers generation.
//!
//! WHAT: Single command that analyzes OpenAPI specs and generates all artifacts together.
//!
//! HOW: Uses foundation_openapi for analysis and generation with intelligent grouping.

use foundation_openapi::{
    UnifiedGenerator,
    unified::{analyze_spec, AnalysisOptions},
};
use std::path::{Path, PathBuf};

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

// ---------------------------------------------------------------------------
// Feature flag fix-up for hierarchical providers
// ---------------------------------------------------------------------------

/// Fix up feature flags for hierarchical providers (e.g., gcp with gcp_admin, gcp_cloudkms, etc.).
///
/// After generating all sub-providers, this function:
/// 1. Scans the output directory for all generated sub-provider directories
/// 2. Updates the parent feature (e.g., "gcp") to include ALL sub-providers
///
/// This is needed because the generator updates Cargo.toml per-spec, but at that time
/// it doesn't know about other sub-providers that will be generated later.
fn fix_hierarchical_features(provider: &str, output_dir: &Path) -> Result<(), BoxedError> {
    use std::collections::BTreeSet;

    let provider_dir = output_dir.join(provider);
    if !provider_dir.exists() {
        return Ok(()); // Nothing to fix
    }

    // Collect all sub-provider directories (ones that contain mod.rs)
    let mut sub_providers: BTreeSet<String> = BTreeSet::new();

    if let Ok(entries) = std::fs::read_dir(&provider_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Check if this is a sub-provider (has mod.rs or is a generated group)
                let mod_rs = path.join("mod.rs");
                if mod_rs.exists() {
                    // Skip "shared" directory - it's not a sub-provider
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name != "shared" && name != "clients" && name != "resources" {
                            sub_providers.insert(name.to_string());
                        }
                    }
                }
            }
        }
    }

    if sub_providers.is_empty() {
        return Ok(()); // Nothing to fix
    }

    println!("\n=== Fixing hierarchical feature flags ===");
    println!("Found {} sub-providers for '{}': {:?}", sub_providers.len(), provider, sub_providers);

    // Update Cargo.toml
    let cargo_toml_path = output_dir
        .ancestors()
        .nth(2)
        .map(|p| p.join("Cargo.toml"))
        .unwrap_or_else(|| PathBuf::from("backends/foundation_deployment/Cargo.toml"));

    if !cargo_toml_path.exists() {
        return Err(format!("Cargo.toml not found at {}", cargo_toml_path.display()).into());
    }

    let content = std::fs::read_to_string(&cargo_toml_path)
        .map_err(|e| format!("Failed to read Cargo.toml: {}", e))?;

    let mut doc: toml::Value = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse Cargo.toml: {}", e))?;

    let features = doc
        .get_mut("features")
        .and_then(|v| v.as_table_mut())
        .ok_or_else(|| "Missing [features] section in Cargo.toml")?;

    // The parent feature name (e.g., "gcp")
    let parent_feature = provider.replace('-', "_");

    // First, remove stale sub-provider feature definitions
    // (features that exist in Cargo.toml but no longer have a directory)
    let keys_to_remove: Vec<String> = features
        .keys()
        .filter(|k| k.starts_with(&format!("{}_", parent_feature)))
        .filter(|k| {
            // Check if this is a sub-provider feature (exactly 2 parts: provider_subprovider)
            let parts: Vec<&str> = k.split('_').collect();
            if parts.len() == 2 {
                // This is a sub-provider feature - check if directory exists
                !sub_providers.contains(&parts[1].to_string())
            } else {
                // This is a group-level feature (e.g., gcp_admin_applications) - also remove
                // if the parent sub-provider doesn't exist
                parts.len() > 2 && !sub_providers.contains(&parts[1].to_string())
            }
        })
        .cloned()
        .collect();

    for key in &keys_to_remove {
        features.remove(key);
    }

    if !keys_to_remove.is_empty() {
        println!("Removed {} stale feature(s): {:?}", keys_to_remove.len(), keys_to_remove);
    }

    // Build the array of sub-provider features
    let sub_provider_features: Vec<String> = sub_providers
        .iter()
        .map(|name| format!("{}_{}", parent_feature, name))
        .collect();

    // Update or create the parent feature
    let parent_feature_array: toml::Value = toml::Value::Array(
        sub_provider_features.iter().map(|f| toml::Value::String(f.clone())).collect()
    );
    features.insert(parent_feature.clone(), parent_feature_array);

    // Serialize back to TOML
    let output = toml::to_string_pretty(&doc)
        .map_err(|e| format!("Failed to serialize Cargo.toml: {}", e))?;

    std::fs::write(&cargo_toml_path, output + "\n")
        .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;

    println!("Updated '{}' feature to include {} sub-providers", parent_feature, sub_provider_features.len());

    Ok(())
}

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
                    )
                    .arg(
                        clap::Arg::new("spec")
                            .long("spec")
                            .short('s')
                            .help("Filter to specific spec (e.g., 'admin' for gcp/admin). Only applies to multi-spec providers")
                            .value_name("SPEC"),
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
            let _with_features = sub_matches.get_flag("features");
            let min_group_size = sub_matches
                .get_one::<String>("min-group-size")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(10);
            let max_group_size = sub_matches
                .get_one::<String>("max-group-size")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(200);
            let spec_filter = sub_matches.get_one::<String>("spec").cloned();

            let options = AnalysisOptions {
                min_group_size,
                max_group_size,
            };

            println!("\n=== Generating for '{}' ===", provider);
            if let Some(ref spec_filter) = spec_filter {
                println!("Filtering to spec: {}", spec_filter);
            }

            // Discover all OpenAPI specs for this provider from artefacts directory
            let artefacts_dir = PathBuf::from("artefacts/cloud_providers");
            let provider_dir = artefacts_dir.join(provider);

            let mut specs: Vec<(String, PathBuf)> = Vec::new();

            if provider_dir.join("openapi.json").exists() {
                // Single spec file (cloudflare, fly_io, etc.)
                specs.push((provider.clone(), provider_dir.join("openapi.json")));
            } else if provider_dir.join(format!("{}.json", provider)).exists() {
                // Named spec file
                specs.push((provider.clone(), provider_dir.join(format!("{}.json", provider))));
            } else {
                // Multiple subdirectories (GCP-like structure) - find all openapi.json files recursively
                fn find_specs(
                    dir: &Path,
                    base: &Path,
                    specs: &mut Vec<(String, PathBuf)>,
                ) -> Result<(), std::io::Error> {
                    if let Ok(entries) = std::fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_dir() {
                                let spec_path = path.join("openapi.json");
                                if spec_path.exists() {
                                    let api_name = path
                                        .strip_prefix(base)
                                        .map(|p| p.to_string_lossy().replace('\\', "/"))
                                        .unwrap_or_else(|_| path.file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("unknown")
                                            .to_string());
                                    specs.push((api_name, spec_path));
                                }
                                // Recurse into subdirectory for nested specs
                                find_specs(&path, base, specs)?;
                            }
                        }
                    }
                    Ok(())
                }

                if provider_dir.exists() {
                    find_specs(&provider_dir, &provider_dir, &mut specs)?;
                    specs.sort_by(|a, b| a.0.cmp(&b.0));
                }
            }

            if specs.is_empty() {
                return Err(format!("Provider '{}' not found in artefacts or has no OpenAPI specs", provider).into());
            }

            // Apply spec filter if provided
            if let Some(spec_filter) = spec_filter {
                let original_count = specs.len();
                specs.retain(|(api_name, _)| {
                    // Match if the api_name equals the filter or ends with the filter
                    // e.g., filter "admin" matches "admin" or "gcp/admin"
                    api_name.as_str() == spec_filter.as_str() || api_name.ends_with(&format!("/{}", spec_filter))
                });
                if specs.is_empty() {
                    return Err(format!(
                        "No spec matching '{}' found for provider '{}'. Available specs: {}",
                        spec_filter,
                        provider,
                        specs.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ")
                    ).into());
                }
                println!("Filtered from {} to {} spec(s)", original_count, specs.len());
            }

            println!("Found {} spec(s) to generate", specs.len());

            for (api_name, spec_path) in &specs {
                println!("\n  Processing {} ({})", api_name, spec_path.display());

                let spec_content = std::fs::read_to_string(spec_path)
                    .map_err(|e| format!("Failed to read spec at {}: {}", spec_path.display(), e))?;

                if dry_run {
                    let analysis = analyze_spec(&spec_content, api_name, &options)
                        .map_err(|e| format!("Analysis failed: {}", e))?;
                    println!("    Groups: {}", analysis.groups.len());
                    continue;
                }

                // Use full provider path for sub-providers (e.g., "gcp/admin")
                let gen_provider = if specs.len() > 1 {
                    format!("{}/{}", provider, api_name)
                } else {
                    provider.clone()
                };

                let generator = UnifiedGenerator::new(output_dir.clone());
                generator.generate(&gen_provider, &spec_content, &options)?;
                println!("    Generated: {}", gen_provider);
            }

            if dry_run {
                println!("\n=== Dry Run Complete ===");
                return Ok(());
            }

            // Post-step: Fix up feature flags for multi-spec providers ONLY
            // When generating gcp/admin, gcp/cloudkms, etc., the parent "gcp" feature
            // should include ALL sub-providers (gcp_admin, gcp_cloudkms, etc.)
            // This scans the output directory for all existing sub-providers.
            // Only run this for multi-spec providers (specs.len() > 1).
            if specs.len() > 1 {
                fix_hierarchical_features(&provider, &output_dir)?;
            }

            println!("\n=== Generation Complete ===");
            println!("Output directory: {}", output_dir.display());

            // Auto-fix common issues (unused imports, snake_case, etc.)
            println!("\n=== Running cargo fix ===");
            let feature_name = provider.replace('-', "_").replace('/', "_");
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
