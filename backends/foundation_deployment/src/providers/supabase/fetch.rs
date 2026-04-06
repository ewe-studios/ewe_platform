//! Supabase `` `OpenAPI` `` spec fetcher.
//!
//! WHY: Supabase is an open-source Firebase alternative providing database,
//! auth, and storage backends for deployments.
//!
//! WHAT: Fetches the Supabase Management API `` `OpenAPI` `` spec and writes it to
//! the provider's output directory.
//!
//! HOW: Delegates to `standard::fetch::fetch_standard_spec` for HTTP download.
//! Provides `process_spec` for post-fetch extraction.

use crate::error::DeploymentError;
use crate::providers::openapi::{self, ProcessedSpec};
use crate::providers::standard;
use foundation_core::valtron::StreamIterator;
use std::path::PathBuf;

/// Supabase Management API `` `OpenAPI` `` spec URL.
pub const SPEC_URL: &str = "https://api.supabase.com/api/v1-json";

/// Provider identifier.
pub const PROVIDER_NAME: &str = "supabase";

/// Fetch the Supabase `` `OpenAPI` `` spec.
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
    standard::fetch::fetch_standard_spec(PROVIDER_NAME, SPEC_URL, output_dir)
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
