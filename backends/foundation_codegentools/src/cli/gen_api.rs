//! WHY: Unified generator command that combines types, clients, and providers generation.
//!
//! WHAT: Single command that analyzes OpenAPI specs and generates all artifacts together.
//!
//! HOW: Uses foundation_openapi for analysis and generation with intelligent grouping.

use foundation_openapi::{
    unified::{analyze_spec, AnalysisOptions},
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

// ---------------------------------------------------------------------------
// Provider output paths
// ---------------------------------------------------------------------------

/// Providers that have been split into their own crate.
/// Key = provider name, Value = crate root (relative to workspace or absolute).
const SPLIT_OUT_PROVIDERS: &[(&str, &str)] = &[
    ("cloudflare", "backends/foundation_deployment_cloudflare"),
    ("hetzner", "backends/foundation_deployment_hetzner"),
    ("digitalocean", "backends/foundation_deployment_digitalocean"),
    ("docker", "backends/foundation_deployment_docker"),
    ("stripe", "backends/foundation_deployment_stripe"),
    ("supabase", "backends/foundation_deployment_supabase"),
    ("neon", "backends/foundation_deployment_neon"),
    ("planetscale", "backends/foundation_deployment_planetscale"),
    // Artefact/provider name differs from the crate suffix; `cargo fix`
    // resolves the crate via `split_out_crate_name()`.
    ("fly_io", "backends/foundation_deployment_flyio"),
    ("prisma_postgres", "backends/foundation_deployment_prisma"),
];

/// Resolve the split-out crate *name* (`foundation_deployment_*`) from its
/// registered path, so `cargo fix -p <crate>` targets the real package even when
/// the provider/artefact name differs from the crate suffix (fly_io → flyio,
/// prisma_postgres → prisma).
fn split_out_crate_name(provider: &str) -> Option<String> {
    for (name, crate_path) in SPLIT_OUT_PROVIDERS {
        if *name == provider {
            return std::path::Path::new(crate_path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());
        }
    }
    None
}

/// Return `(crate_root, is_split_out)` for a provider name.
fn provider_crate_info(provider: &str) -> (PathBuf, bool) {
    for (name, crate_path) in SPLIT_OUT_PROVIDERS {
        if *name == provider {
            return (PathBuf::from(crate_path), true);
        }
    }
    (
        PathBuf::from("backends/foundation_deployment/src/providers"),
        false,
    )
}

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
                    if name != "shared"
                        && name != "clients"
                        && name != "resources"
                        && seen.insert(name.to_string())
                    {
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
    writeln!(
        out,
        "//! DO NOT EDIT MANUALLY - regeneration will overwrite this file."
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#![cfg(feature = \"{}\")]", feature_name).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "// Shared module - re-exports common API types").unwrap();
    writeln!(out, "pub mod shared;").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "// Sub-providers are auto-generated and conditionally compiled."
    )
    .unwrap();
    writeln!(
        out,
        "// Each sub-provider has its own feature flag (e.g., gcp_admin, gcp_cloudkms)."
    )
    .unwrap();
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

    std::fs::write(provider_dir.join("mod.rs"), out + "\n").map_err(|e| {
        format!(
            "Failed to write {}: {}",
            provider_dir.join("mod.rs").display(),
            e
        )
    })?;

    println!(
        "Generated {}/mod.rs with {} sub-provider(s)",
        provider,
        sub_providers.len()
    );

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
/// When `regenerated` is provided, only clears/rebuilds features for those
/// sub-providers. When None, clears ALL provider features and rebuilds from
/// the current directory structure.
fn fix_hierarchical_features(
    provider: &str,
    provider_dir: &Path,
    regenerated: &BTreeSet<String>,
    cargo_toml_path: &Path,
) -> Result<(), BoxedError> {
    if !provider_dir.exists() {
        return Ok(());
    }

    // Collect all sub-provider directories (ones that contain mod.rs)
    let mut sub_providers: BTreeSet<String> = BTreeSet::new();

    if let Ok(entries) = std::fs::read_dir(provider_dir) {
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
    println!(
        "Found {} sub-provider(s): {:?}",
        sub_providers.len(),
        sub_providers
    );

    if !cargo_toml_path.exists() {
        return Err(format!("Cargo.toml not found at {}", cargo_toml_path.display()).into());
    }

    let content = std::fs::read_to_string(&cargo_toml_path)
        .map_err(|e| format!("Failed to read Cargo.toml: {}", e))?;

    let mut doc: toml::Value =
        toml::from_str(&content).map_err(|e| format!("Failed to parse Cargo.toml: {}", e))?;

    let features = doc
        .get_mut("features")
        .and_then(|v| v.as_table_mut())
        .ok_or_else(|| "Missing [features] section in Cargo.toml")?;

    let provider_snake = provider.replace('-', "_");
    let all_feature = format!("{}_all", provider_snake);
    let prefix = format!("{}_", provider_snake);

    // Step 0: Clear provider features.
    // If `regenerated` is non-empty (partial regen via --spec), only clear
    // features for those sub-providers. Otherwise clear ALL provider features.
    let full_regeneration = regenerated.is_empty();
    let old_features: Vec<String> = features
        .keys()
        .filter(|k| {
            if *k == &provider_snake {
                return full_regeneration;
            }
            if !k.starts_with(&prefix) {
                return false;
            }
            if full_regeneration {
                return true; // Clear everything for full regeneration
            }
            // For partial regen, only clear features belonging to regenerated sub-providers
            let without_prefix = k.strip_prefix(&prefix).unwrap_or("");
            regenerated.iter().any(|sub| {
                without_prefix == *sub // bare sub-provider feature
                    || without_prefix.starts_with(&format!("{sub}_")) // nested feature
                    || without_prefix.starts_with(&format!("{sub}/"))
            })
        })
        .cloned()
        .collect();
    for old in &old_features {
        features.remove(old);
    }
    if !old_features.is_empty() {
        println!(
            "Removed {} stale feature(s) for regenerated sub-providers",
            old_features.len()
        );
    }

    // Also clean up references to cleared features from the base provider feature
    // and the *_all feature, since they still reference removed sub-features.
    if !full_regeneration && !old_features.is_empty() {
        let removed_set: BTreeSet<String> = old_features.iter().cloned().collect();

        // Clean base provider feature (e.g., gcp = [...])
        if let Some(toml::Value::Array(ref mut arr)) = features.get_mut(&provider_snake) {
            arr.retain(|v| v.as_str().map(|s| !removed_set.contains(s)).unwrap_or(true));
        }

        // Clean *_all feature (e.g., gcp_all = [...])
        if let Some(toml::Value::Array(ref mut arr)) = features.get_mut(&all_feature) {
            arr.retain(|v| v.as_str().map(|s| !removed_set.contains(s)).unwrap_or(true));
        }
    }

    // Step 1: Ensure base provider feature exists (e.g., cloudflare = [])
    //    This enables the provider module without any specific API.
    features
        .entry(provider_snake.clone())
        .or_insert(toml::Value::Array(vec![]));

    // Step 2: Detect if this is a hierarchical provider (nested sub-providers).
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
        fix_hierarchical_features_recursive(
            &provider_snake,
            &all_feature,
            &sub_providers,
            features,
            regenerated,
        );
    } else {
        // Flat: cloudflare_all = ["cloudflare_access", "cloudflare_workers", ...]
        fix_flat_features(
            &provider_snake,
            &all_feature,
            &sub_providers,
            features,
            regenerated,
        );
    }

    // Debug: show gcp_run feature
    if let Some(run_feat) = features.get("gcp_run") {
        println!("  gcp_run feature after fix: {:?}", run_feat);
    }

    let output = toml::to_string_pretty(&doc)
        .map_err(|e| format!("Failed to serialize Cargo.toml: {}", e))?;

    std::fs::write(&cargo_toml_path, output + "\n")
        .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;

    Ok(())
}

/// Fix features for hierarchical providers (e.g., gcp with nested groups).
/// Only rebuilds features for sub-providers in `regenerated` (if non-empty).
fn fix_hierarchical_features_recursive(
    provider_snake: &str,
    all_feature: &str,
    sub_providers: &BTreeSet<String>,
    features: &mut toml::Table,
    regenerated: &BTreeSet<String>,
) {
    let full_regeneration = regenerated.is_empty();

    let output_dir = PathBuf::from("backends/foundation_deployment/src/providers");
    let provider_dir = output_dir.join(provider_snake);

    let mut all_group_features: BTreeSet<String> = BTreeSet::new();

    for sub in sub_providers {
        let safe_sub = camel_to_snake(sub);
        if !full_regeneration && !regenerated.contains(&safe_sub) {
            // Still collect existing features for partial regen
            for key in features.keys() {
                if key.starts_with(&format!("{}_{safe_sub}_", provider_snake))
                    || key.starts_with(&format!("{all_feature}_{safe_sub}"))
                {
                    // Keep existing features
                }
            }
            // Collect existing group feature for this sub
            let group_feature = format!("{all_feature}_{safe_sub}");
            if features.contains_key(&group_feature) {
                all_group_features.insert(group_feature);
            }
            continue;
        }

        // Scan the sub-provider directory for nested group directories
        let sub_dir = provider_dir.join(&safe_sub);
        let mut group_names: Vec<String> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&sub_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("mod.rs").exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        // Skip shared/clients/resources meta-directories
                        // Also skip self-named directories (e.g., gcp/admin/admin/)
                        if name != "shared"
                            && name != "clients"
                            && name != "resources"
                            && name != &safe_sub
                        {
                            group_names.push(camel_to_snake(name));
                        }
                    }
                }
            }
        }
        group_names.sort();
        group_names.dedup();

        // Always include the self-named group feature since the generator creates
        // a module at {sub}/{sub}/mod.rs guarded by gcp_{sub}_{sub}.
        let self_group_feature = format!("{provider_snake}_{safe_sub}_{safe_sub}");
        let mut group_apis: Vec<String> = vec![self_group_feature.clone()];
        features
            .entry(self_group_feature.clone())
            .or_insert(toml::Value::Array(vec![]));

        // Generate individual group features: gcp_{sub}_{group} = []
        for group in &group_names {
            let feature_name = format!("{provider_snake}_{safe_sub}_{group}");
            features
                .entry(feature_name.clone())
                .or_insert(toml::Value::Array(vec![]));
            group_apis.push(feature_name);
        }

        // Generate group-level feature: gcp_all_{sub} = [gcp_{sub}_{group1}, ...]
        let group_feature = format!("{all_feature}_{safe_sub}");
        if group_apis.is_empty() {
            features.insert(group_feature.clone(), toml::Value::Array(vec![]));
        } else {
            features.insert(
                group_feature.clone(),
                toml::Value::Array(
                    group_apis
                        .iter()
                        .map(|f| toml::Value::String(f.clone()))
                        .collect(),
                ),
            );
        }
        all_group_features.insert(group_feature.clone());

        // Generate sub-provider base feature: gcp_{sub} = ["gcp", gcp_{sub}_{sub}]
        // This allows depending on a whole sub-provider (e.g., gcp_run)
        // and transitively enables the provider module and all its endpoints.
        let sub_base_feature = format!("{provider_snake}_{safe_sub}");
        features.insert(
            sub_base_feature,
            toml::Value::Array(vec![
                toml::Value::String(provider_snake.to_string()),
                toml::Value::String(self_group_feature),
            ]),
        );
    }

    // Ensure *_all feature includes all group-level features
    let existing_all = if full_regeneration {
        BTreeSet::new()
    } else {
        features
            .get(all_feature)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default()
    };

    let merged: Vec<String> = existing_all.union(&all_group_features).cloned().collect();
    features.insert(
        all_feature.to_string(),
        toml::Value::Array(merged.into_iter().map(|f| toml::Value::String(f)).collect()),
    );
}

/// Fix features for flat providers (e.g., cloudflare, stripe, supabase).
/// Only rebuilds features for sub-providers in `regenerated` (if non-empty).
fn fix_flat_features(
    provider_snake: &str,
    all_feature: &str,
    sub_providers: &BTreeSet<String>,
    features: &mut toml::Table,
    regenerated: &BTreeSet<String>,
) {
    let full_regeneration = regenerated.is_empty();

    // Ensure individual API features exist
    for sub in sub_providers {
        let feature_name = format!("{}_{}", provider_snake, camel_to_snake(sub));
        if !full_regeneration && !regenerated.contains(&camel_to_snake(sub)) {
            continue;
        }
        features
            .entry(feature_name)
            .or_insert(toml::Value::Array(vec![]));
    }

    // Collect all individual API features for this provider
    let prefix = format!("{}_", provider_snake);
    let existing_apis: BTreeSet<String> = features
        .keys()
        .filter(|k| k.starts_with(&prefix) && !k.contains("_all"))
        .map(|k| k.clone())
        .collect();

    // Merge into *_all
    let existing_all = if full_regeneration {
        BTreeSet::new()
    } else {
        features
            .get(all_feature)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default()
    };

    let merged: Vec<String> = existing_all.union(&existing_apis).cloned().collect();
    features.insert(
        all_feature.to_string(),
        toml::Value::Array(merged.into_iter().map(|f| toml::Value::String(f)).collect()),
    );
}

// ---------------------------------------------------------------------------
// CLI registration
// ---------------------------------------------------------------------------

/// Register the `gen_api` subcommand.
pub fn register(cmd: clap::Command) -> clap::Command {
    cmd.subcommand(command())
}

/// Build a standalone `gen_api` command (for use as a binary).
#[must_use]
pub fn command() -> clap::Command {
    clap::Command::new("gen_api")
        .about("Unified generator - generates types, clients, and providers from OpenAPI specs")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(crate::cli::normalize::command())
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
                            .help("Override where generated files go. Defaults to the provider's own layout: its own crate for split-out providers, foundation_deployment/src/providers otherwise")
                            .value_name("DIR"),
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
                    )
                    // Selection (spec-56 feature 00 §1). Absent, everything is
                    // generated — which is what every existing provider expects.
                    .arg(
                        clap::Arg::new("include-path")
                            .long("include-path")
                            .help("Generate only this path; repeatable. Globs: * one segment, ** many")
                            .value_name("GLOB")
                            .action(clap::ArgAction::Append),
                    )
                    .arg(
                        clap::Arg::new("include-tag")
                            .long("include-tag")
                            .help("Generate only operations with this tag; repeatable")
                            .value_name("TAG")
                            .action(clap::ArgAction::Append),
                    )
                    .arg(
                        clap::Arg::new("include-operation")
                            .long("include-operation")
                            .help("Generate only this operationId; repeatable")
                            .value_name("ID")
                            .action(clap::ArgAction::Append),
                    )
                    .arg(
                        clap::Arg::new("spec-version")
                            .long("spec-version")
                            .help("Pin the spec revision (info.version); a mismatch fails the run")
                            .value_name("VERSION"),
                    )
                    .arg(
                        clap::Arg::new("api-version")
                            .long("api-version")
                            .help("Pin the vendor's API version (hetzner v1, linode v4, digitalocean v2)")
                            .value_name("VERSION"),
                    )
                    .arg(
                        clap::Arg::new("check")
                            .long("check")
                            .help("Fail if the committed output is not what this run would generate. Writes nothing")
                            .action(clap::ArgAction::SetTrue),
                    )
                    .arg(
                        clap::Arg::new("buildrs-overwrite")
                            .long("buildrs-overwrite")
                            .help("Replace the crate's build.rs. Without this, an existing one is left alone — it is a source file people edit")
                            .action(clap::ArgAction::SetTrue),
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
    )
}

/// Run the `gen_api` command.
pub fn run(matches: &clap::ArgMatches) -> Result<(), BoxedError> {
    match matches.subcommand() {
        // raw spec -> canonical artefact (spec-56 decision 05). Runs before
        // `generate`, which assumes canonical input — so a generation bug and a
        // normalization bug are never the same bug.
        Some(("normalize", sub_matches)) => crate::cli::normalize::run(sub_matches),
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
                println!(
                    "\n  Shared Resources ({} types):",
                    analysis.shared_resources.len()
                );
                for resource in &analysis.shared_resources {
                    println!("    - {}", resource);
                }
            }

            println!("\n=== Analysis Complete ===");

            Ok(())
        }
        Some(("generate", sub_matches)) => {
            let provider = sub_matches.get_one::<String>("provider").unwrap();
            // Only an *explicit* -o overrides the provider's own layout. The flag
            // carries a default that equals the monolith root, so treating "has a
            // value" as "the user chose one" would silently generate split-out
            // crates into foundation_deployment.
            let output_override = matches!(
                sub_matches.value_source("output-dir"),
                Some(clap::parser::ValueSource::CommandLine)
            )
            .then(|| {
                sub_matches
                    .get_one::<String>("output-dir")
                    .map(PathBuf::from)
                    .expect("clap guarantees a value for a flag it sourced")
            });
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

            // Selection + pins (spec-56 feature 00). Left unset, `Selection::all()`
            // generates everything — which is what cloudflare/gcp/stripe expect.
            let mut selection = foundation_openapi::Selection::all();
            if let Some(v) = sub_matches.get_many::<String>("include-path") {
                selection = selection.paths(v.cloned().collect::<Vec<_>>());
            }
            if let Some(v) = sub_matches.get_many::<String>("include-tag") {
                selection = selection.tags(v.cloned().collect::<Vec<_>>());
            }
            if let Some(v) = sub_matches.get_many::<String>("include-operation") {
                selection = selection.operations(v.cloned().collect::<Vec<_>>());
            }
            let spec_version = sub_matches.get_one::<String>("spec-version").cloned();
            let api_version = sub_matches.get_one::<String>("api-version").cloned();
            let check_only = sub_matches.get_flag("check");
            let buildrs_overwrite = sub_matches.get_flag("buildrs-overwrite");

            let options = AnalysisOptions {
                min_group_size,
                max_group_size,
            };

            println!("\n=== Generating for '{}' ===", provider);
            if let Some(ref spec_filter) = spec_filter {
                println!("Filtering to spec: {}", spec_filter);
            }
            if !selection.is_all() {
                for (label, id) in [
                    ("paths", "include-path"),
                    ("tags", "include-tag"),
                    ("operations", "include-operation"),
                ] {
                    if let Some(values) = sub_matches.get_many::<String>(id) {
                        let values: Vec<&str> = values.map(String::as_str).collect();
                        println!("Selected {}: {}", label, values.join(", "));
                    }
                }
            }

            // ── Determine output paths ──
            let (default_root, is_split) = provider_crate_info(provider);
            let crate_root = output_override.unwrap_or(default_root);
            let output_dir = crate_root.clone();
            // The provider's source directory. The generator appends `generated/`
            // internally, so all files land under <provider_dir>/generated/.
            //  - monolith: <crate_root>/<provider>/generated/  (e.g. foundation_deployment/src/providers/cloudflare/generated/)
            //  - split:    <crate_root>/src/generated/          (generated into crate src/generated/)
            let actual_provider_dir = if is_split {
                crate_root.join("src")
            } else {
                output_dir.join(provider)
            };
            let generated_dir = actual_provider_dir.join("generated");

            println!("Output directory: {}", generated_dir.display());
            if is_split {
                println!("Mode: split-out crate");
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
                specs.push((
                    provider.clone(),
                    provider_dir.join(format!("{}.json", provider)),
                ));
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
                                        .unwrap_or_else(|_| {
                                            path.file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or("unknown")
                                                .to_string()
                                        });
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
                return Err(format!(
                    "Provider '{}' not found in artefacts or has no OpenAPI specs",
                    provider
                )
                .into());
            }

            // Apply spec filter if provided
            let is_multi_spec_provider = specs.len() > 1;
            if let Some(spec_filter) = spec_filter {
                let original_count = specs.len();
                specs.retain(|(api_name, _)| {
                    api_name.as_str() == spec_filter.as_str()
                        || api_name.ends_with(&format!("/{}", spec_filter))
                });
                if specs.is_empty() {
                    return Err(format!(
                        "No spec matching '{}' found for provider '{}'. Available specs: {}",
                        spec_filter,
                        provider,
                        specs
                            .iter()
                            .map(|(n, _)| n.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                    .into());
                }
                println!(
                    "Filtered from {} to {} spec(s)",
                    original_count,
                    specs.len()
                );
            }

            println!("Found {} spec(s) to generate", specs.len());

            // Track which sub-providers were regenerated in this run
            let mut regenerated: BTreeSet<String> = BTreeSet::new();

            for (api_name, spec_path) in &specs {
                println!("\n  Processing {} ({})", api_name, spec_path.display());

                // Use full provider path for sub-providers (e.g., "gcp/admin").
                let safe_api_name = camel_to_snake(api_name);
                let gen_provider = if is_multi_spec_provider {
                    format!("{}/{}", provider, safe_api_name)
                } else {
                    provider.clone()
                };

                // One declaration, shared with build.rs callers — so a selection
                // means the same thing from either entry point, and the generator
                // is never shown the paths we did not ask for.
                let mut codegen = crate::codegen::generate()
                    .provider(&gen_provider)
                    .spec(spec_path)
                    .select(selection.clone())
                    .output_dir(output_dir.clone())
                    .group_sizes(min_group_size, max_group_size);
                if is_split {
                    // Split-out crate: generated files go to crate_root/src/,
                    // alongside hand-written client.rs, types.rs, dns_ops.rs.
                    codegen = codegen.provider_dir(actual_provider_dir.clone());
                }
                if let Some(v) = &spec_version {
                    codegen = codegen.spec_version(v);
                }
                if let Some(v) = &api_version {
                    codegen = codegen.api_version(v);
                }

                if dry_run {
                    // Resolve, so the pin and the selection are exercised — a dry
                    // run that skipped them would report on a spec the real run
                    // never sees.
                    let resolved = codegen.resolve()?;
                    let analysis = analyze_spec(
                        &serde_json::to_string(&resolved)?,
                        api_name,
                        &options,
                    )
                    .map_err(|e| format!("Analysis failed: {}", e))?;
                    println!(
                        "    Paths: {} | schemas: {} | groups: {}",
                        resolved
                            .get("paths")
                            .and_then(serde_json::Value::as_object)
                            .map_or(0, serde_json::Map::len),
                        foundation_openapi::component_count(&resolved, "schemas"),
                        analysis.groups.len()
                    );
                    continue;
                }

                if check_only {
                    codegen.check()?;
                    println!("    Up to date: {}", gen_provider);
                    continue;
                }

                let report = codegen.write()?;
                println!(
                    "    Generated: {} ({} paths, {} schemas, {} files)",
                    gen_provider, report.paths_selected, report.schemas, report.files
                );

                // The declaration, written next to the code it describes
                // (feature 00 §4). Split-out crates only: a monolith provider has
                // no crate of its own to carry one.
                if is_split {
                    let build_rs = codegen
                        .clone()
                        .crate_dir(crate_root.clone())
                        .write_build_script(buildrs_overwrite)?;
                    match &build_rs {
                        crate::codegen::BuildScriptOutcome::Created(p) => {
                            println!("    Wrote {}", p.display());
                        }
                        crate::codegen::BuildScriptOutcome::Replaced(p) => {
                            println!("    Replaced {} (--buildrs-overwrite)", p.display());
                        }
                        crate::codegen::BuildScriptOutcome::Kept(p) => {
                            println!(
                                "    Kept {} — pass --buildrs-overwrite to replace it",
                                p.display()
                            );
                        }
                    }
                }

                // Track which sub-provider was regenerated
                regenerated.insert(safe_api_name);
            }

            if check_only {
                println!("\n=== Check Complete — output matches ===");
                return Ok(());
            }

            if dry_run {
                println!("\n=== Dry Run Complete ===");
                return Ok(());
            }

            // Post-step: Fix up feature flags for all providers with sub-groups.
            if is_multi_spec_provider {
                generate_parent_mod_rs(&provider, &generated_dir)?;
            }
            // For split-out crates, the Cargo.toml is at crate_root;
            // for monolith crates, it's 2 levels up from output_dir.
            let cargo_toml_path = if is_split {
                crate_root.join("Cargo.toml")
            } else {
                output_dir
                    .ancestors()
                    .nth(2)
                    .map(|p| p.join("Cargo.toml"))
                    .unwrap_or_else(|| PathBuf::from("backends/foundation_deployment/Cargo.toml"))
            };
            fix_hierarchical_features(
                &provider,
                &generated_dir,
                &regenerated,
                &cargo_toml_path,
            )?;

            println!("\n=== Generation Complete ===");
            println!("Output directory: {}", generated_dir.display());

            // Auto-fix common issues (unused imports, snake_case, etc.)
            println!("\n=== Running cargo fix ===");
            let feature_name = provider.replace('-', "_").replace('/', "_");
            let crate_name = if is_split {
                split_out_crate_name(provider)
                    .unwrap_or_else(|| format!("foundation_deployment_{}", provider))
            } else {
                "foundation_deployment".to_string()
            };
            let fix_output = std::process::Command::new("cargo")
                .args([
                    "fix",
                    "--lib",
                    "-p",
                    &crate_name,
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
