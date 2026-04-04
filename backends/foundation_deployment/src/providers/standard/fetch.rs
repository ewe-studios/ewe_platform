//! Standard HTTP-based OpenAPI spec fetcher.
//!
//! WHY: Most cloud providers expose their OpenAPI specs at a single HTTP
//! endpoint that returns JSON directly. A generic fetcher avoids duplicating
//! the same download-parse-write logic for every provider.
//!
//! WHAT: Downloads an OpenAPI spec from a URL using `curl`, validates that the
//! response is valid JSON, writes `openapi.json` and `_manifest.json` to the
//! provider's output directory.
//!
//! HOW: Uses `foundation_core::valtron::from_future` to wrap the blocking
//! `curl` call, then `execute()` to schedule it on the Valtron thread pool.
//! Returns a `StreamIterator` consistent with other provider fetchers.

use crate::error::DeploymentError;
use foundation_core::valtron::{execute, from_future, StreamIterator, StreamIteratorExt};
use std::path::PathBuf;
use std::process::Command;

/// Fetch an OpenAPI spec from a standard HTTP endpoint.
///
/// # Arguments
///
/// * `provider` - Human-readable provider name (used in logs and manifest)
/// * `url` - URL that returns an OpenAPI JSON spec
/// * `output_dir` - Directory to write `openapi.json` and `_manifest.json`
///
/// # Returns
///
/// A `StreamIterator` that yields `Result<PathBuf, DeploymentError>` when
/// the fetch completes. The `PathBuf` points to the written `openapi.json`.
pub fn fetch_standard_spec(
    provider: &str,
    url: &str,
    output_dir: PathBuf,
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static,
    DeploymentError,
> {
    let provider = provider.to_string();
    let url = url.to_string();

    let future = async move {
        tracing::info!("Fetching {provider} spec from {url}");

        // Use curl for reliable HTTP fetching (avoids SimpleHttpClient WouldBlock
        // issues with large responses).
        let output = Command::new("curl")
            .args(["-sL", &url])
            .output()
            .map_err(|e| {
                DeploymentError::Generic(format!("{provider}: curl failed to execute: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DeploymentError::Generic(format!(
                "{provider}: curl returned non-zero exit code: {stderr}"
            )));
        }

        let body = String::from_utf8(output.stdout).map_err(|e| {
            DeploymentError::Generic(format!("{provider}: response is not valid UTF-8: {e}"))
        })?;

        // Validate JSON
        let json_value: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            DeploymentError::Generic(format!("{provider}: response is not valid JSON: {e}"))
        })?;

        // Create output directory
        std::fs::create_dir_all(&output_dir).map_err(|e| {
            DeploymentError::Generic(format!(
                "{provider}: failed to create output directory {}: {e}",
                output_dir.display()
            ))
        })?;

        // Write openapi.json (pretty-printed)
        let output_path = output_dir.join("openapi.json");
        let pretty = serde_json::to_string_pretty(&json_value).map_err(|e| {
            DeploymentError::Generic(format!("{provider}: failed to serialize JSON: {e}"))
        })?;

        std::fs::write(&output_path, &pretty).map_err(|e| {
            DeploymentError::Generic(format!(
                "{provider}: failed to write {}: {e}",
                output_path.display()
            ))
        })?;

        // Write _manifest.json
        let manifest = serde_json::json!({
            "source": url,
            "fetched_at": chrono::Utc::now().to_rfc3339(),
            "provider": provider,
        });

        let manifest_path = output_dir.join("_manifest.json");
        std::fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest)?,
        )
        .map_err(|e| {
            DeploymentError::Generic(format!(
                "{provider}: failed to write manifest: {e}"
            ))
        })?;

        tracing::info!(
            "{provider} spec saved to {} ({} bytes)",
            output_path.display(),
            pretty.len()
        );

        Ok(output_path)
    };

    let task = from_future(future);

    let stream = execute(task, None)
        .map_err(|e| DeploymentError::Generic(format!("Valtron scheduling failed: {e}")))?;

    Ok(stream.map_pending(|_| ()))
}
