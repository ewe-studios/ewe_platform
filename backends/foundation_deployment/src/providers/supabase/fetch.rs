//! Supabase `` `OpenAPI` `` spec fetcher and normalizer.
//!
//! WHY: Supabase is an open-source Firebase alternative providing database,
//! auth, and storage backends for deployments.
//!
//! WHAT: Fetches the Supabase Management API `` `OpenAPI` `` spec and writes it to
//! the provider's output directory.
//!
//! HOW: Delegates to `standard::fetch::fetch_standard_spec` for HTTP download,
//! then normalizes the spec to add the missing `servers` field.

use crate::error::DeploymentError;
use crate::providers::openapi::{self, ProcessedSpec};
use crate::providers::standard::normalize;
use foundation_core::valtron::{from_future, StreamIterator, StreamIteratorExt};
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
use tracing::info;

/// Supabase Management API base URL.
pub const BASE_URL: &str = "https://api.supabase.com";

/// Supabase Management API `` `OpenAPI` `` spec URL.
pub const SPEC_URL: &str = "https://api.supabase.com/api/v1-json";

/// Provider identifier.
pub const PROVIDER_NAME: &str = "supabase";

/// Fetch the Supabase `` `OpenAPI` `` spec and normalize it.
///
/// After fetching the raw spec, applies normalization to add the `servers` field
/// (Supabase spec has empty `servers: []`).
///
/// # Errors
///
/// Returns `DeploymentError` if the HTTP fetch fails or writing the file fails.
pub fn fetch_supabase_specs(
    output_dir: PathBuf,
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static,
    DeploymentError,
> {
    let output_dir = output_dir.clone();

    let future = async move {
        info!("Fetching Supabase OpenAPI spec from {}", SPEC_URL);

        // Ensure output directory exists
        std::fs::create_dir_all(&output_dir).map_err(|e| {
            DeploymentError::IoError(std::io::Error::other(format!(
                "Failed to create output directory: {e}"
            )))
        })?;

        // Fetch using curl
        let output_path = output_dir.join("openapi.json");
        let output = Command::new("curl")
            .args(["-s", "-o"])
            .arg(&output_path)
            .arg(SPEC_URL)
            .output()
            .map_err(|e| {
                DeploymentError::ProcessFailed {
                    command: format!("curl -o {} {}", output_path.display(), SPEC_URL),
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!("curl execution failed: {e}"),
                }
            })?;

        if !output.status.success() {
            return Err(DeploymentError::ProcessFailed {
                command: format!("curl -o {} {}", output_path.display(), SPEC_URL),
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        // Read, normalize, and write back
        let content = std::fs::read_to_string(&output_path).map_err(|e| {
            DeploymentError::IoError(std::io::Error::other(format!(
                "Failed to read fetched spec: {e}"
            )))
        })?;

        let mut spec: Value = serde_json::from_str(&content).map_err(|e| {
            DeploymentError::ConfigInvalid {
                file: "supabase openapi.json".to_string(),
                reason: format!("Invalid JSON: {e}"),
            }
        })?;

        normalize_supabase_spec(&mut spec);

        let normalized = serde_json::to_string_pretty(&spec).map_err(|e| {
            DeploymentError::ConfigInvalid {
                file: "supabase openapi.json".to_string(),
                reason: format!("Serialization failed: {e}"),
            }
        })?;

        std::fs::write(&output_path, normalized).map_err(|e| {
            DeploymentError::IoError(std::io::Error::other(format!(
                "Failed to write normalized spec: {e}"
            )))
        })?;

        info!("Successfully fetched and normalized Supabase spec to {:?}", output_path);
        Ok(output_path)
    };

    let task = from_future(future);
    let stream = foundation_core::valtron::execute(task, None)
        .map_err(|e| DeploymentError::Generic(format!("Valtron scheduling failed: {e}")))?;

    Ok(stream.map_pending(|_| ()))
}

/// Normalize a Supabase OpenAPI spec.
///
/// Supabase spec has `servers: []` (empty array) - this adds the proper server URL.
pub fn normalize_supabase_spec(spec: &mut Value) {
    normalize::ensure_servers(spec, BASE_URL);
}

/// Process a fetched Supabase spec.
///
/// # Returns
///
/// Returns a `ProcessedSpec` with extracted endpoints and metadata.
#[must_use]
pub fn process_spec(spec: &serde_json::Value) -> ProcessedSpec {
    openapi::process_spec(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_correct() {
        assert_eq!(PROVIDER_NAME, "supabase");
        assert!(SPEC_URL.starts_with("https://"));
        assert!(SPEC_URL.contains("supabase.com"));
    }

    #[test]
    fn process_spec_extracts_endpoints() {
        let spec = serde_json::json!({
            "info": { "version": "1.0.0", "title": "Supabase Management API" },
            "paths": {
                "/v1/projects": {
                    "get": { "operationId": "getProjects", "summary": "List projects" },
                    "post": { "operationId": "createProject", "summary": "Create a project" }
                },
                "/v1/organizations": {
                    "get": { "operationId": "getOrganizations", "summary": "List orgs" }
                }
            }
        });

        let processed = process_spec(&spec);
        assert_eq!(processed.version, Some("1.0.0".to_string()));
        assert_eq!(processed.endpoints.as_ref().map(Vec::len), Some(3));
    }
}
