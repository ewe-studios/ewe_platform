//! GCP `OpenAPI` spec fetcher.
//!
//! WHY: GCP has 300+ APIs, each with its own OpenAPI spec.
//!
//! WHAT: Discovers all available GCP API specs from the artefacts directory
//! and returns them for batch generation.
//!
//! HOW: GLOBs for all `*/openapi.json` files under the GCP artefacts directory.
//! Unlike other providers that fetch from a single URL, GCP specs are pre-fetched
//! and stored in subdirectories (one per API).

use crate::error::DeploymentError;
use foundation_core::valtron::{from_future, StreamIterator, StreamIteratorExt};
use std::path::{Path, PathBuf};
use tracing::info;

/// Provider identifier used in output paths and logs.
pub const PROVIDER_NAME: &str = "gcp";

/// Base directory for GCP artefacts (relative to project root).
const ARTEFACTS_BASE: &str = "artefacts/cloud_providers/gcp";

/// Recursively find all `openapi.json` files under a directory.
///
/// Returns a sorted Vec of (api_name, spec_path) tuples.
fn find_all_specs(base_dir: &Path) -> Result<Vec<(String, PathBuf)>, std::io::Error> {
    let mut specs: Vec<(String, PathBuf)> = Vec::new();

    fn walk(dir: &Path, base_dir: &Path, specs: &mut Vec<(String, PathBuf)>) -> Result<(), std::io::Error> {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Check if this directory has an openapi.json
                    let spec_path = path.join("openapi.json");
                    if spec_path.exists() {
                        // Use the full relative path from base as the API name
                        let api_name = path
                            .strip_prefix(base_dir)
                            .map(|p| p.to_string_lossy().replace('\\', "/"))
                            .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));
                        specs.push((api_name, spec_path));
                    }
                    // Recurse into subdirectory
                    walk(&path, base_dir, specs)?;
                }
            }
        }
        Ok(())
    }

    walk(base_dir, base_dir, &mut specs)?;
    specs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(specs)
}

/// Fetch all GCP OpenAPI specs.
///
/// Discovers all `*/openapi.json` files under the GCP artefacts directory.
///
/// # Arguments
///
/// * `_output_dir` - Ignored for GCP; specs are read from artefacts directory.
///
/// # Returns
///
/// `StreamIterator` yielding `Result<(String, PathBuf), DeploymentError>` where
/// the String is the API name and PathBuf is the spec file path.
///
/// # Errors
///
/// Returns `DeploymentError` if directory reading fails.
pub fn fetch_gcp_specs(
    _output_dir: PathBuf,
) -> Result<
    impl StreamIterator<D = Result<(String, PathBuf), DeploymentError>, P = ()> + Send + 'static,
    DeploymentError,
> {
    let artefacts_path = PathBuf::from(ARTEFACTS_BASE);

    let future = async move {
        info!("Discovering GCP API specs in {}", ARTEFACTS_BASE);

        let specs = find_all_specs(&artefacts_path).map_err(|e| DeploymentError::Io {
            path: artefacts_path.display().to_string(),
            source: e,
        })?;

        info!("Found {} GCP API specs", specs.len());

        // Return all specs as a single batch result
        Ok(specs)
    };

    let task = from_future(future);
    let stream = foundation_core::valtron::execute(task, None)
        .map_err(|e| DeploymentError::Generic(format!("Valtron scheduling failed: {e}")))?;

    // Flatten the batch result into individual items
    Ok(stream.flat_map_next(|result| match result {
        Ok(specs) => specs.into_iter().map(Ok).collect::<Vec<_>>(),
        Err(e) => vec![Err(e)],
    })
    .map_pending(|_| ()))
}

/// Process a GCP spec into version, endpoints, and content hash.
///
/// # Returns
///
/// Returns a `ProcessedSpec` with extracted endpoints and metadata.
#[must_use]
pub fn process_spec(spec: &serde_json::Value) -> crate::providers::openapi::ProcessedSpec {
    crate::providers::openapi::process_spec(spec)
}
