//! `` `MongoDB` `` Atlas `` `OpenAPI` `` spec fetcher.
//!
//! WHY: `` `MongoDB` `` Atlas is a managed database service commonly used as a
//! deployment backend for applications.
//!
//! WHAT: Fetches the `` `MongoDB` `` Atlas Admin API v2 `` `OpenAPI` `` spec and writes it
//! to the provider's output directory.
//!
//! HOW: Delegates to `standard::fetch::fetch_standard_spec` for HTTP download.
//! Provides `process_spec` for post-fetch extraction.

use crate::error::DeploymentError;
use crate::providers::openapi::{self, ProcessedSpec};
use crate::providers::standard;
use foundation_core::valtron::StreamIterator;
use std::path::PathBuf;

/// `` `MongoDB` `` Atlas Admin API v2 `` `OpenAPI` `` spec URL.
pub const SPEC_URL: &str = "https://www.mongodb.com/docs/api/doc/atlas-admin-api-v2.json";

/// Provider identifier.
pub const PROVIDER_NAME: &str = "mongodb_atlas";

/// Fetch the `` `MongoDB` `` Atlas `` `OpenAPI` `` spec.
///
/// # Errors
///
/// Returns `DeploymentError` if the HTTP fetch fails or writing the file fails.
pub fn fetch_mongodb_atlas_specs(
    output_dir: PathBuf,
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static,
    DeploymentError,
> {
    standard::fetch::fetch_standard_spec(PROVIDER_NAME, SPEC_URL, output_dir)
}

/// Process a fetched `` `MongoDB` `` Atlas spec.
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
        assert_eq!(PROVIDER_NAME, "mongodb_atlas");
        assert!(SPEC_URL.starts_with("https://"));
        assert!(SPEC_URL.contains("mongodb.com"));
    }

    #[test]
    fn process_spec_extracts_atlas_endpoints() {
        let spec = serde_json::json!({
            "info": { "version": "2.0", "title": "MongoDB Atlas Admin API" },
            "paths": {
                "/api/atlas/v2/groups": {
                    "get": { "operationId": "listProjects", "summary": "List projects" }
                },
                "/api/atlas/v2/groups/{groupId}/clusters": {
                    "get": { "operationId": "listClusters", "summary": "List clusters" },
                    "post": { "operationId": "createCluster", "summary": "Create a cluster" }
                }
            }
        });

        let processed = process_spec(&spec);
        assert_eq!(processed.version, Some("2.0".to_string()));
        assert_eq!(processed.endpoints.as_ref().map(Vec::len), Some(3));
    }
}
