//! WHY: Unified generator command that combines types, clients, and providers generation.
//!
//! WHAT: Single command that analyzes OpenAPI specs and generates all artifacts together.
//!
//! HOW: Uses foundation_openapi for analysis and generation with intelligent grouping.

use foundation_openapi::{
    UnifiedGenerator,
    unified::{analyze_spec, AnalysisOptions},
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

// ---------------------------------------------------------------------------
// Feature flag fix-up for hierarchical providers
// ---------------------------------------------------------------------------

/// Generate the parent provider's `mod.rs` that declares all sub-provider modules.
///
/// BUG FIX: The generator writes each sub-provider's mod.rs (e.g. gcp/admin/mod.rs)
/// but never writes the top-level gcp/mod.rs with `pub mod admin; pub mod run;` etc.
/// Without this, the sub-providers can't be imported even though their code exists.
fn generate_parent_mod_rs(provider: &str, output_dir: &Path) -> Result<(), BoxedError> {
    use std::fmt::Write as FmtWrite;

    let provider_dir = output_dir.join(provider);
    if !provider_dir.exists() {
        return Ok(());
    }

    // Collect all sub-provider directories
    let mut sub_providers: Vec<(String, String)> = Vec::new(); // (display_name, safe_name)
    let mut seen = std::collections::HashSet::new();

    if let Ok(entries) = std::fs::read_dir(&provider_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("mod.rs").exists() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name != "shared" && name != "clients" && name != "resources" && seen.insert(name.to_string()) {
                        let safe = camel_to_snake(name);
                        sub_providers.push((name.to_string(), safe));
                    }
                }
            }
        }
    }

    if sub_providers.is_empty() {
        return Ok(());
    }

    sub_providers.sort_by(|a, b| a.1.cmp(&b.1));

    let feature_name = provider.replace('-', "_").replace('/', "_");

    let mut out = String::new();
    writeln!(out, "//! Google Cloud Platform provider.").unwrap();
    writeln!(out, "//!").unwrap();
    writeln!(out, "//! Generated sub-providers for each GCP API.").unwrap();
    writeln!(out, "//! DO NOT EDIT MANUALLY - regeneration will overwrite this file.").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#![cfg(feature = \"{}\")]", feature_name).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "// Shared module - re-exports common API types").unwrap();
    writeln!(out, "pub mod shared;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "// Sub-providers are auto-generated and conditionally compiled.").unwrap();
    writeln!(out, "// Each sub-provider has its own feature flag (e.g., gcp_admin, gcp_cloudkms).").unwrap();
    writeln!(out).unwrap();

    for (display, safe) in &sub_providers {
        // Rename directory if it uses camelCase
        if display != safe {
            let old = provider_dir.join(display);
            let new = provider_dir.join(safe);
            if old.exists() && !new.exists() {
                let _ = std::fs::rename(&old, &new);
            }
        }
        let sub_feature = format!("{}_{}", feature_name, safe);
        writeln!(out, "#[cfg(feature = \"{}\")]", sub_feature).unwrap();
        writeln!(out, "pub mod {};", safe).unwrap();
    }

    std::fs::write(provider_dir.join("mod.rs"), out + "\n")
        .map_err(|e| format!("Failed to write {}: {}", provider_dir.join("mod.rs").display(), e))?;

    println!("Generated {}/mod.rs with {} sub-provider(s)", provider, sub_providers.len());

    Ok(())
}

/// Convert camelCase to snake_case.
fn camel_to_snake(name: &str) -> String {
    let mut result = String::new();
    let mut prev_was_upper_or_digit = false;
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 && !prev_was_upper_or_digit {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
            prev_was_upper_or_digit = true;
        } else if c.is_numeric() {
            result.push(c);
            prev_was_upper_or_digit = true;
        } else {
            result.push(c);
            prev_was_upper_or_digit = false;
        }
    }
    result
}

/// Fix up feature flags for all providers so they have the standard pattern:
///   - `provider = []` — enables the module but no specific APIs
///   - `provider_<api>` — individual API feature
///   - `provider_all = ["provider_<api>", ...]` — enables all APIs
///
/// For hierarchical providers like gcp, also creates group-level features:
///   - `gcp_all_<group> = ["gcp_<group>_<api>", ...]`
///   - `gcp_all = ["gcp_all_<group>", ...]`
///
/// IMPORTANT: This is additive only — it never removes existing features.
fn fix_hierarchical_features(provider: &str, output_dir: &Path) -> Result<(), BoxedError> {
    let provider_dir = output_dir.join(provider);
    if !provider_dir.exists() {
        return Ok(());
    }

    // Collect all sub-provider directories (ones that contain mod.rs)
    let mut sub_providers: BTreeSet<String> = BTreeSet::new();

    if let Ok(entries) = std::fs::read_dir(&provider_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let mod_rs = path.join("mod.rs");
                if mod_rs.exists() {
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
        return Ok(());
    }

    println!("\n=== Fixing feature flags for '{}' ===", provider);
    println!("Found {} sub-provider(s): {:?}", sub_providers.len(), sub_providers);

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

    let provider_snake = provider.replace('-', "_");
    let all_feature = format!("{}_all", provider_snake);

    // 1. Ensure base provider feature is empty (e.g., cloudflare = [])
    //    This enables the provider module without any specific API.
    match features.get(&provider_snake) {
        Some(v) if v.as_array().map_or(false, |a| a.is_empty()) => {}
        _ => {
            features.insert(provider_snake.clone(), toml::Value::Array(vec![]));
            println!("Set '{} = []' (empty base feature)", provider_snake);
        }
    }

    // 2. Detect if this is a hierarchical provider (nested sub-providers).
    //    Check if any sub-provider directory itself contains mod.rs files
    //    that declare further sub-modules.
    let is_hierarchical = sub_providers.iter().any(|sub| {
        let sub_dir = provider_dir.join(sub);
        if let Ok(entries) = std::fs::read_dir(sub_dir) {
            entries.flatten().any(|e| {
                let p = e.path();
                p.is_dir() && p.join("mod.rs").exists()
            })
        } else {
            false
        }
    });

    if is_hierarchical {
        // Hierarchical: gcp_all = ["gcp_all_admin", ...], gcp_all_admin = ["gcp_admin_applications", ...]
        fix_hierarchical_features_recursive(&provider_snake, &all_feature, &sub_providers, features);
    } else {
        // Flat: cloudflare_all = ["cloudflare_access", "cloudflare_workers", ...]
        fix_flat_features(&provider_snake, &all_feature, &sub_providers, features);
    }

    let output = toml::to_string_pretty(&doc)
        .map_err(|e| format!("Failed to serialize Cargo.toml: {}", e))?;

    std::fs::write(&cargo_toml_path, output + "\n")
        .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;

    Ok(())
}

/// Fix features for hierarchical providers (e.g., gcp with nested groups).
fn fix_hierarchical_features_recursive(
    provider_snake: &str,
    all_feature: &str,
    sub_providers: &BTreeSet<String>,
    features: &mut toml::value::Table,
) {
    // Add group-level features: gcp_all_admin, gcp_all_cloudkms, etc.
    for sub in sub_providers {
        let group_feature = format!("{}_{}", all_feature, sub);
        if !features.contains_key(&group_feature) {
            // Find all API features belonging to this group
            let group_apis: Vec<String> = features.keys()
                .filter(|k| k.starts_with(&format!("{}_", provider_snake)) && !k.contains("_all_"))
                .filter(|k| {
                    let api_part = k.strip_prefix(&format!("{}_", provider_snake)).unwrap_or("");
                    api_part.starts_with(&format!("{}/", sub)) || api_part.starts_with(&format!("{}_", sub))
                })
                .map(|k| k.clone())
                .collect();

            if group_apis.is_empty() {
                features.insert(group_feature.clone(), toml::Value::Array(vec![]));
            } else {
                features.insert(group_feature.clone(), toml::Value::Array(
                    group_apis.iter().map(|f| toml::Value::String(f.clone())).collect()
                ));
            }
        }
    }

    // Ensure *_all feature includes all group-level features
    let existing_all = features
        .get(all_feature)
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<BTreeSet<_>>())
        .unwrap_or_default();

    let group_features: BTreeSet<String> = sub_providers
        .iter()
        .map(|name| format!("{}_{}", all_feature, name))
        .collect();

    let merged: Vec<String> = existing_all.union(&group_features).cloned().collect();
    features.insert(all_feature.to_string(), toml::Value::Array(
        merged.iter().map(|f| toml::Value::String(f.clone())).collect()
    ));

    println!("Updated '{}_all' feature to include {} group(s)", provider_snake, merged.len());
}

/// Fix features for flat providers (e.g., cloudflare, stripe, supabase).
fn fix_flat_features(
    provider_snake: &str,
    all_feature: &str,
    sub_providers: &BTreeSet<String>,
    features: &mut toml::value::Table,
) {
    // Ensure individual API features exist
    for sub in sub_providers {
        let feature_name = format!("{}_{}", provider_snake, sub);
        if !features.contains_key(&feature_name) {
            features.insert(feature_name.clone(), toml::Value::Array(vec![]));
        }
    }

    // Collect all individual API features for this provider
    let prefix = format!("{}_", provider_snake);
    let all_api_features: BTreeSet<String> = features.keys()
        .filter(|k| k.starts_with(&prefix) && !k.contains("_all"))
        .map(|k| k.clone())
        .collect();

    // Merge into *_all
    let existing_all = features
        .get(all_feature)
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<BTreeSet<_>>())
        .unwrap_or_default();

    let merged: Vec<String> = existing_all.union(&all_api_features).cloned().collect();
    features.insert(all_feature.to_string(), toml::Value::Array(
        merged.iter().map(|f| toml::Value::String(f.clone())).collect()
    ));

    println!("Updated '{}_all' feature to include {} API(s)", provider_snake, merged.len());
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
            let is_multi_spec_provider = specs.len() > 1;
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

                // Use full provider path for sub-providers (e.g., "gcp/admin").
                // Check is_multi_spec_provider (before filtering), NOT specs.len(),
                // so that filtering to one spec doesn't cause it to overwrite the parent.
                let safe_api_name = camel_to_snake(api_name);
                let gen_provider = if is_multi_spec_provider {
                    format!("{}/{}", provider, safe_api_name)
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

            // Post-step: Fix up feature flags for all providers with sub-groups.
            // This ensures *_all features exist for both multi-spec (gcp) and
            // single-spec multi-group (cloudflare, stripe, etc.) providers.
            if is_multi_spec_provider {
                generate_parent_mod_rs(&provider, &output_dir)?;
            }
            fix_hierarchical_features(&provider, &output_dir)?;

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
