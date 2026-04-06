//! `` `PlanetScale` `` `` `OpenAPI` `` spec fetcher.
//!
//! WHY: `` `PlanetScale` `` is a serverless `` `MySQL` `` database platform used as a
//! deployment backend.
//!
//! WHAT: Fetches the `` `PlanetScale` `` API `` `OpenAPI` `` spec and writes it to the
//! provider's output directory.
//!
//! HOW: Delegates to `standard::fetch::fetch_standard_spec` for HTTP download.
//! Provides `process_spec` for post-fetch extraction.

use crate::error::DeploymentError;
use crate::providers::openapi::{self, ProcessedSpec};
use crate::providers::standard;
use foundation_core::valtron::StreamIterator;
use std::path::PathBuf;

/// `` `PlanetScale` `` API `` `OpenAPI` `` spec URL.
pub const SPEC_URL: &str = "https://api.planetscale.com/v1/openapi-spec";

/// Provider identifier.
pub const PROVIDER_NAME: &str = "planetscale";

/// Fetch the `` `PlanetScale` `` `` `OpenAPI` `` spec.
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
    standard::fetch::fetch_standard_spec(PROVIDER_NAME, SPEC_URL, output_dir)
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
