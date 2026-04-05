//! Standard OpenAPI spec fetcher for providers with simple HTTP GET requirements.
//!
//! Uses curl via std::process::Command to fetch OpenAPI specs from public URLs.
//! This is a simple, reliable approach for providers without custom auth requirements.

use crate::error::DeploymentError;
use foundation_core::valtron::{from_future, StreamIterator, StreamIteratorExt};
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, info};

/// Fetch a standard OpenAPI spec from a URL and write it to the output directory.
///
/// This function is used by providers that have publicly accessible OpenAPI specs
/// without custom authentication requirements.
///
/// # Arguments
///
/// * `provider_name` - Name of the provider (used for logging and filename)
/// * `spec_url` - URL to fetch the OpenAPI spec from
/// * `output_dir` - Directory to write the spec to
///
/// # Returns
///
/// StreamIterator yielding Result<PathBuf, DeploymentError> when complete.
pub fn fetch_standard_spec(
    provider_name: &str,
    spec_url: &str,
    output_dir: PathBuf,
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static,
    DeploymentError,
> {
    let provider_name = provider_name.to_string();
    let spec_url = spec_url.to_string();

    let future = async move {
        info!("Fetching {} OpenAPI spec from {}", provider_name, spec_url);

        // Ensure output directory exists
        std::fs::create_dir_all(&output_dir).map_err(|e| {
            DeploymentError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create output directory: {e}"),
            ))
        })?;

        // Fetch using curl
        let output_path = output_dir.join("openapi.json");
        let output = Command::new("curl")
            .args(["-s", "-o"])
            .arg(&output_path)
            .arg(&spec_url)
            .output()
            .map_err(|e| {
                DeploymentError::ProcessFailed {
                    command: format!("curl -o {} {}", output_path.display(), spec_url),
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!("curl execution failed: {e}"),
                }
            })?;

        if !output.status.success() {
            return Err(DeploymentError::ProcessFailed {
                command: format!("curl -o {} {}", output_path.display(), spec_url),
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        // Read and validate as JSON
        let content = std::fs::read_to_string(&output_path).map_err(|e| {
            DeploymentError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to read fetched spec: {e}"),
            ))
        })?;

        let spec: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            DeploymentError::ConfigInvalid {
                file: format!("{} OpenAPI spec", provider_name),
                reason: format!("Invalid JSON: {e}"),
            }
        })?;

        debug!("Successfully wrote {} spec to {:?}", provider_name, output_path);

        // Extract version for logging
        let version = crate::providers::openapi::extract_version(&spec)
            .unwrap_or_else(|| "unknown".to_string());

        info!(
            "Successfully fetched {} spec (version: {}) to {:?}",
            provider_name, version, output_path
        );

        Ok(output_path)
    };

    let task = from_future(future);
    let stream = foundation_core::valtron::execute(task, None)
        .map_err(|e| DeploymentError::Generic(format!("Valtron scheduling failed: {e}")))?;

    Ok(stream.map_pending(|_| ()))
}
