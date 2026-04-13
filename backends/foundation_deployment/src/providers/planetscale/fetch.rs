//! `` `PlanetScale` `` `` `OpenAPI` `` spec fetcher and normalizer.
//!
//! WHY: `` `PlanetScale` `` is a serverless `` `MySQL` `` database platform used as a
//! deployment backend.
//!
//! WHAT: Fetches the `` `PlanetScale` `` API `` `OpenAPI` `` spec and writes it to the
//! provider's output directory.
//!
//! HOW: Delegates to `standard::fetch::fetch_standard_spec` for HTTP download,
//! then normalizes the spec from Swagger 2.0 to `OpenAPI` 3.x format.

use crate::error::DeploymentError;
use crate::providers::openapi::{self, ProcessedSpec};
use foundation_core::valtron::{from_future, StreamIterator, StreamIteratorExt};
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::process::Command;
use tracing::info;

/// `` `PlanetScale` `` API base URL.
pub const BASE_URL: &str = "https://api.planetscale.com";

/// `` `PlanetScale` `` API `` `OpenAPI` `` spec URL.
pub const SPEC_URL: &str = "https://api.planetscale.com/v1/openapi-spec";

/// Provider identifier.
pub const PROVIDER_NAME: &str = "planetscale";

/// Fetch the `` `PlanetScale` `` `` `OpenAPI` `` spec and normalize it.
///
/// After fetching the raw spec, applies normalization to convert from Swagger 2.0
/// to `OpenAPI` 3.x format (host + schemes + basePath → servers).
///
/// # Errors
///
/// Returns `DeploymentError` if the HTTP fetch fails or writing the file fails.
pub fn fetch_planetscale_specs(
    output_dir: PathBuf,
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static,
    DeploymentError,
> {
    let future = async move {
        info!("Fetching PlanetScale OpenAPI spec from {}", SPEC_URL);

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
                file: "planetscale openapi.json".to_string(),
                reason: format!("Invalid JSON: {e}"),
            }
        })?;

        normalize_planetscale_spec(&mut spec);

        let normalized = serde_json::to_string_pretty(&spec).map_err(|e| {
            DeploymentError::ConfigInvalid {
                file: "planetscale openapi.json".to_string(),
                reason: format!("Serialization failed: {e}"),
            }
        })?;

        std::fs::write(&output_path, normalized).map_err(|e| {
            DeploymentError::IoError(std::io::Error::other(format!(
                "Failed to write normalized spec: {e}"
            )))
        })?;

        info!("Successfully fetched and normalized PlanetScale spec to {:?}", output_path);
        Ok(output_path)
    };

    let task = from_future(future);
    let stream = foundation_core::valtron::execute(task, None)
        .map_err(|e| DeploymentError::Generic(format!("Valtron scheduling failed: {e}")))?;

    Ok(stream.map_pending(|_| ()))
}

/// Normalize a `PlanetScale` Swagger 2.0 spec to `OpenAPI` 3.x format.
///
/// Converts:
/// - `host` + `schemes` + `basePath` → `servers`
/// - `definitions` → `components/schemas`
/// - `swagger: "2.0"` → `openapi: "3.0.3"`
///
/// # Panics
///
/// Panics if the spec structure is malformed (e.g., `components` is not an object).
/// This is expected to never happen for valid `PlanetScale` specs.
pub fn normalize_planetscale_spec(spec: &mut Value) {
    let Some(obj) = spec.as_object_mut() else { return };

    // Convert host + schemes + basePath to servers
    let host = obj.remove("host").and_then(|v| v.as_str().map(String::from));
    let schemes = obj.remove("schemes").and_then(|v| v.as_array().cloned());
    let base_path = obj.remove("basePath").and_then(|v| v.as_str().map(String::from));

    if let Some(host) = host {
        let scheme = schemes
            .as_ref()
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
            .unwrap_or("https");
        let url = format!("{}://{}/{}", scheme, host, base_path.as_deref().unwrap_or("").trim_start_matches('/'));
        obj.insert("servers".to_string(), serde_json::json!([{"url": url.trim_end_matches('/')}]));
    }

    // Convert definitions to components/schemas
    if let Some(definitions) = obj.remove("definitions") {
        let components = obj.entry("components").or_insert_with(|| Value::Object(Map::new()));
        components
            .as_object_mut()
            .unwrap()
            .insert("schemas".to_string(), definitions);
    }

    // Update swagger version to openapi
    if let Some(swagger) = obj.get("swagger") {
        if swagger.as_str() == Some("2.0") {
            obj.insert("openapi".to_string(), Value::String("3.0.3".to_string()));
            obj.remove("swagger");
        }
    }

    // Remove consumes/produces (not used in OpenAPI 3.x)
    obj.remove("consumes");
    obj.remove("produces");
}

/// Process a fetched `` `PlanetScale` `` spec.
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
        assert_eq!(PROVIDER_NAME, "planetscale");
        assert!(SPEC_URL.starts_with("https://"));
        assert!(SPEC_URL.contains("planetscale.com"));
    }

    #[test]
    fn process_spec_extracts_database_endpoints() {
        let spec = serde_json::json!({
            "info": { "version": "v1", "title": "PlanetScale API" },
            "paths": {
                "/organizations/{name}/databases": {
                    "get": { "operationId": "list-databases", "summary": "List databases" },
                    "post": { "operationId": "create-database", "summary": "Create a database" }
                },
                "/organizations/{name}/databases/{db_name}/branches": {
                    "get": { "operationId": "list-branches", "summary": "List branches" }
                }
            }
        });

        let processed = process_spec(&spec);
        assert_eq!(processed.version, Some("v1".to_string()));
        assert_eq!(processed.endpoints.as_ref().map(Vec::len), Some(3));
    }
}
