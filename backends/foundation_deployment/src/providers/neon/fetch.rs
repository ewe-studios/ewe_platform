//! Neon OpenAPI spec fetcher.
//!
//! WHY: Neon is a serverless Postgres platform used as a deployment backend.
//!
//! WHAT: Fetches the Neon v2 API OpenAPI spec and writes it to the provider's
//! output directory.
//!
//! HOW: Delegates to `standard::fetch::fetch_standard_spec` for HTTP download.
//! Provides `process_spec` for post-fetch extraction. Neon's spec is
//! well-structured with endpoints for projects, branches, endpoints,
//! databases, roles, and operations.

use crate::error::DeploymentError;
use crate::providers::openapi::{self, ProcessedSpec};
use crate::providers::standard;
use foundation_core::valtron::StreamIterator;
use std::path::PathBuf;

/// Neon v2 API OpenAPI spec URL.
pub const SPEC_URL: &str = "https://neon.com/api_spec/release/v2.json";

/// Provider identifier.
pub const PROVIDER_NAME: &str = "neon";

/// Fetch the Neon OpenAPI spec.
pub fn fetch_neon_specs(
    output_dir: PathBuf,
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static,
    DeploymentError,
> {
    standard::fetch::fetch_standard_spec(PROVIDER_NAME, SPEC_URL, output_dir)
}

/// Process a fetched Neon spec.
pub fn process_spec(spec: &serde_json::Value) -> ProcessedSpec {
    openapi::process_spec(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_correct() {
        assert_eq!(PROVIDER_NAME, "neon");
        assert!(SPEC_URL.starts_with("https://"));
        assert!(SPEC_URL.contains("neon.com"));
    }

    #[test]
    fn process_spec_extracts_neon_endpoints() {
        let spec = serde_json::json!({
            "info": { "version": "2.0.0", "title": "Neon API" },
            "paths": {
                "/projects": {
                    "get": { "operationId": "listProjects", "summary": "List projects" },
                    "post": { "operationId": "createProject", "summary": "Create a project" }
                },
                "/projects/{project_id}/branches": {
                    "get": { "operationId": "listBranches", "summary": "List branches" },
                    "post": { "operationId": "createBranch", "summary": "Create a branch" }
                },
                "/projects/{project_id}/endpoints": {
                    "get": { "operationId": "listEndpoints", "summary": "List endpoints" }
                }
            }
        });

        let processed = process_spec(&spec);
        assert_eq!(processed.version, Some("2.0.0".to_string()));
        assert_eq!(processed.endpoints.as_ref().map(Vec::len), Some(5));
    }
}
