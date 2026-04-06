//! Fly.io `OpenAPI` spec fetcher.
//!
//! WHY: Fly.io is a deployment target for applications via the Machines API.
//!
//! WHAT: Fetches the Fly.io Machines API `OpenAPI` spec from a single URL and
//! writes it to the provider's output directory.
//!
//! HOW: Delegates to `standard::fetch::fetch_standard_spec` for the actual
//! HTTP download. Provides `process_spec` for post-fetch extraction of
//! version, endpoints, and content hash.


use crate::error::DeploymentError;
use crate::providers::openapi::{self, ProcessedSpec};
use crate::providers::standard;
use foundation_core::valtron::StreamIterator;
use std::path::PathBuf;

/// Fly.io Machines API `OpenAPI` spec URL.
pub const SPEC_URL: &str = "https://docs.machines.dev/spec/openapi3.json";

/// Provider identifier used in output paths and logs.
pub const PROVIDER_NAME: &str = "fly_io";

/// Fetch the Fly.io `OpenAPI` spec.
///
/// # Errors
///
/// Returns `DeploymentError` if the HTTP fetch fails or writing the file fails.
///
/// # Returns
///
/// Returns a `StreamIterator` that yields `Result<PathBuf>` when complete.
pub fn fetch_fly_io_specs(
    output_dir: PathBuf,
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static,
    DeploymentError,
> {
    standard::fetch::fetch_standard_spec(PROVIDER_NAME, SPEC_URL, output_dir)
}

/// Process a fetched Fly.io spec into version, endpoints, and content hash.
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
        assert_eq!(PROVIDER_NAME, "fly_io");
        assert!(SPEC_URL.starts_with("https://"));
        assert!(SPEC_URL.contains("machines.dev"));
    }

    #[test]
    fn process_spec_extracts_version() {
        let spec = serde_json::json!({
            "info": { "version": "1.0.0", "title": "Fly Machines" },
            "paths": {
                "/apps": {
                    "get": { "operationId": "Apps_list", "summary": "List apps" }
                }
            }
        });

        let processed = process_spec(&spec);
        assert_eq!(processed.version, Some("1.0.0".to_string()));
        assert!(processed.endpoints.is_some());
        assert_eq!(processed.endpoints.as_ref().map(Vec::len), Some(1));
        assert!(!processed.content_hash.is_empty());
    }

    #[test]
    fn process_spec_handles_minimal_spec() {
        let spec = serde_json::json!({});
        let processed = process_spec(&spec);
        assert_eq!(processed.version, None);
        assert_eq!(processed.endpoints, None);
        assert!(!processed.content_hash.is_empty());
    }
}
