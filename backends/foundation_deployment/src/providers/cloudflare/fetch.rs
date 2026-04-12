//! Cloudflare `OpenAPI` spec fetcher.
//!
//! WHY: Cloudflare API schemas are hosted in a GitHub repository,
//! requiring git clone to fetch.
//!
//! WHAT: Clones the api-schemas repo and extracts relevant API spec files.
//!
//! HOW: Uses `foundation_core::valtron::from_future` to wrap the sync git clone
//! operation, then `execute()` to get a `StreamIterator`. The stream yields
//! the result when the clone + file processing completes.

use crate::error::DeploymentError;
use foundation_core::valtron::{execute, from_future, StreamIterator, StreamIteratorExt};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Cloudflare API schemas GitHub repository.
pub const CLOUDFLARE_API_SCHEMAS_URL: &str = "https://github.com/cloudflare/api-schemas";

/// Relevant Cloudflare API spec file prefixes.
const RELEVANT_PREFIXES: &[&str] = &[
    "workers",
    "kv",
    "d1",
    "r2",
    "queues",
    "vectorize",
    "hyperdrive",
    "pages",
    "tenant",
    "accounts",
];

/// Progress states for Cloudflare fetch (always () - no intermediate progress).
pub type CloudflareFetchPending = ();

/// Normalize a Cloudflare schema name to a Rust-safe identifier.
///
/// Replaces `-`, `.`, `@` with `_` and ensures the name starts with a letter.
fn normalize_schema_name(name: &str) -> String {
    let normalized = name
        .replace(['-', '.', '@'], "_");

    // Ensure name starts with a letter (prepend underscore if it starts with digit)
    if normalized.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("_{normalized}")
    } else {
        normalized
    }
}

/// Normalize all schema names in components/schemas and update $ref references.
fn normalize_cloudflare_spec(spec: &mut Value) {
    let Some(obj) = spec.as_object_mut() else { return };

    // First, collect schema renames
    let mut renames: Vec<(String, String)> = Vec::new();

    if let Some(components) = obj.get_mut("components").and_then(|c| c.as_object_mut()) {
        if let Some(schemas) = components.get_mut("schemas").and_then(|s| s.as_object_mut()) {
            let keys: Vec<String> = schemas.keys().cloned().collect();

            for key in keys {
                let normalized = normalize_schema_name(&key);
                if normalized != key {
                    renames.push((key.clone(), normalized));
                }
            }

            // Rename schemas
            for (old, new) in &renames {
                if let Some(schema) = schemas.remove(old) {
                    schemas.insert(new.clone(), schema);
                }
            }
        }
    }

    // Build a rename map for $ref updates
    let rename_map: Map<String, Value> = renames
        .iter()
        .map(|(old, new)| {
            let old_ref = format!("#/components/schemas/{old}");
            let new_ref = format!("#/components/schemas/{new}");
            (old_ref.clone(), Value::String(new_ref))
        })
        .collect();

    // Update all $ref references in paths and schemas
    if !rename_map.is_empty() {
        let mut spec_value = Value::Object(std::mem::take(obj));
        update_refs_in_value(&mut spec_value, &rename_map);
        *obj = spec_value.as_object_mut().unwrap().clone();
    }
}

/// Recursively update $ref fields in a JSON value.
fn update_refs_in_value(value: &mut Value, rename_map: &Map<String, Value>) {
    match value {
        Value::Object(obj) => {
            for (key, val) in obj.iter_mut() {
                if key == "$ref" {
                    if let Value::String(ref_path) = val {
                        if let Some(new_ref) = rename_map.get(ref_path) {
                            *val = new_ref.clone();
                        }
                    }
                } else {
                    update_refs_in_value(val, rename_map);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                update_refs_in_value(item, rename_map);
            }
        }
        _ => {}
    }
}

/// Fetch Cloudflare specs by cloning the GitHub repo.
///
/// Returns a `StreamIterator` that yields the result when complete.
/// The work runs on the Valtron thread pool.
///
/// # Arguments
///
/// * `temp_dir` - Temporary directory for cloning
/// * `output_dir` - Output directory for consolidated spec (`artefacts/cloud_providers/cloudflare/`)
///
/// # Returns
///
/// `StreamIterator` yielding `Result<PathBuf, DeploymentError>`.
///
/// # Errors
///
/// Returns `DeploymentError::ProcessFailed` if git clone fails,
/// `DeploymentError::IoError` if file operations fail,
/// or `DeploymentError::JsonInvalid` if spec parsing fails.
pub fn fetch_cloudflare_specs(
    temp_dir: PathBuf,
    output_dir: PathBuf,
) -> Result<impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static, DeploymentError> {
    let future = async move {
        tracing::info!("Cloning Cloudflare API schemas repository...");
        tracing::debug!("Temp dir: {:?}", temp_dir);
        tracing::debug!("Output dir: {:?}", output_dir);

        // Clone the repo with depth=1 for speed
        let clone_output = Command::new("git")
            .args(["clone", "--depth", "1", CLOUDFLARE_API_SCHEMAS_URL, "cloudflare-schemas"])
            .current_dir(&temp_dir)
            .output()
            .map_err(|e| DeploymentError::Generic(format!("Failed to clone repo: {e}")))?;

        tracing::debug!("Git clone completed, status: {}", clone_output.status);

        if !clone_output.status.success() {
            let stderr = String::from_utf8_lossy(&clone_output.stderr);
            tracing::error!("Git clone stderr: {}", stderr);
            return Err(DeploymentError::Generic(format!(
                "Git clone failed: {stderr}"
            )));
        }

        let source_dir = temp_dir.join("cloudflare-schemas");
        tracing::debug!("Source dir: {:?}", source_dir);

        // Find relevant Cloudflare API spec files
        let spec_files = find_cloudflare_api_files(&source_dir);
        tracing::info!("Found {} relevant Cloudflare API spec files", spec_files.len());
        for (name, _) in &spec_files {
            tracing::debug!("  Found spec file: {}", name);
        }

        // Create destination directory
        std::fs::create_dir_all(&output_dir).map_err(|e| DeploymentError::Generic(format!(
            "Failed to create output directory: {e}"
        )))?;

        // Cloudflare api-schemas repo has a single consolidated openapi.json
        // Just copy it directly without wrapping
        let output_path = output_dir.join("openapi.json");

        if spec_files.len() == 1 {
            // Single file - copy, normalize, and write
            let (_, src_path) = &spec_files[0];
            let content = std::fs::read_to_string(src_path).map_err(|e| DeploymentError::Generic(format!(
                "Failed to read spec file: {e}"
            )))?;

            let mut spec: Value = serde_json::from_str(&content).map_err(|e| DeploymentError::Generic(format!(
                "Failed to parse spec: {e}"
            )))?;

            // Normalize schema names and update $refs
            normalize_cloudflare_spec(&mut spec);

            let normalized = serde_json::to_string_pretty(&spec).map_err(|e| DeploymentError::Generic(format!(
                "Failed to serialize normalized spec: {e}"
            )))?;

            std::fs::write(&output_path, normalized).map_err(|e| DeploymentError::Generic(format!(
                "Failed to write output file: {e}"
            )))?;
            tracing::info!("Copied and normalized Cloudflare spec to: {}", output_path.display());
        } else {
            // Multiple files - consolidate by merging paths and schemas
            let mut merged_paths = serde_json::Map::new();
            let mut merged_schemas = serde_json::Map::new();

            for (name, src_path) in &spec_files {
                tracing::debug!("Reading spec file: {:?}", src_path);
                let content = std::fs::read_to_string(src_path).map_err(|e| DeploymentError::Generic(format!(
                    "Failed to read {name}: {e}"
                )))?;

                if let Ok(spec) = serde_json::from_str::<Value>(&content) {
                    if let Some(obj) = spec.as_object() {
                        // Merge paths
                        if let Some(paths) = obj.get("paths").and_then(|p| p.as_object()) {
                            for (path, path_item) in paths {
                                merged_paths.insert(path.clone(), path_item.clone());
                            }
                        }

                        // Merge components/schemas
                        if let Some(schemas) = obj.get("components").and_then(|c| c.get("schemas")).and_then(|s| s.as_object()) {
                            for (schema_name, schema) in schemas {
                                // Normalize and prefix the schema name
                                let normalized = normalize_schema_name(schema_name);
                                let prefixed_name = format!("{}_{}", name.replace(".json", ""), normalized);
                                merged_schemas.insert(prefixed_name, schema.clone());
                            }
                        }
                    }
                }
            }

            // Build consolidated OpenAPI spec
            let consolidated = serde_json::json!({
                "openapi": "3.0.0",
                "info": {
                    "title": "Cloudflare API",
                    "version": "consolidated",
                    "description": "Consolidated Cloudflare API spec"
                },
                "servers": [
                    {"url": "https://api.cloudflare.com/client/v4"}
                ],
                "paths": merged_paths,
                "components": {
                    "schemas": merged_schemas
                }
            });

            let json = serde_json::to_string_pretty(&consolidated).map_err(|e| {
                DeploymentError::Generic(format!("Failed to serialize JSON: {e}"))
            })?;
            std::fs::write(&output_path, json).map_err(|e| DeploymentError::Generic(format!(
                "Failed to write output file: {e}"
            )))?;
            tracing::info!("Written consolidated Cloudflare spec to: {}", output_path.display());
        }

        // Write manifest
        let manifest = serde_json::json!({
            "provider": "cloudflare",
            "source": CLOUDFLARE_API_SCHEMAS_URL,
            "fetched_at": chrono::Utc::now().to_rfc3339(),
            "spec_files": ["openapi.json"],
        });

        let manifest_path = output_dir.join("_manifest.json");
        std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?).map_err(|e| {
            DeploymentError::Generic(format!("Failed to write manifest: {e}"))
        })?;

        // Clean up cloned repo
        let _ = Command::new("rm")
            .args(["-rf", "cloudflare-schemas"])
            .current_dir(&temp_dir)
            .output();

        tracing::info!("Cloudflare spec saved to: {}", output_path.display());
        Ok(output_path)
    };

    let task = from_future(future);

    let stream = execute(task, None)
        .map_err(|e| DeploymentError::Generic(format!("Valtron scheduling failed: {e}")))?;

    Ok(stream.map_pending(|_| ()))
}

/// Find relevant Cloudflare API spec files in the cloned repo.
fn find_cloudflare_api_files(source: &Path) -> Vec<(String, PathBuf)> {
    let mut files = Vec::new();

    for entry in walkdir::WalkDir::new(source).into_iter().flatten() {
        if entry.path().extension() == Some("json".as_ref()) {
            let file_name = entry.path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();

            if file_name == "openapi.json" || RELEVANT_PREFIXES.iter().any(|p| file_name.starts_with(p)) {
                files.push((file_name.clone(), entry.path().to_path_buf()));
            }
        }
    }

    files
}
