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
use serde_json::Value;
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
            // Single file - copy directly
            let (_, src_path) = &spec_files[0];
            std::fs::copy(src_path, &output_path).map_err(|e| DeploymentError::Generic(format!(
                "Failed to copy spec file: {e}"
            )))?;
            tracing::info!("Copied Cloudflare spec to: {}", output_path.display());
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
                                let prefixed_name = format!("{}_{schema_name}", name.replace(".json", ""));
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
