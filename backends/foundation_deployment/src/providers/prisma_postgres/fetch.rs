//! Prisma Postgres `` `OpenAPI` `` spec fetcher.
//!
//! WHY: Prisma Postgres is a managed database service for application
//! deployments.
//!
//! WHAT: Fetches the Prisma Postgres API `` `OpenAPI` `` spec and writes it to the
//! provider's output directory.
//!
//! HOW: Delegates to `standard::fetch::fetch_standard_spec` for HTTP download.
//! Provides `process_spec` for post-fetch extraction.

use crate::error::DeploymentError;
use crate::providers::openapi::{self, ProcessedSpec};
use crate::providers::standard;
use foundation_core::valtron::StreamIterator;
use std::path::PathBuf;

/// Prisma Postgres API `` `OpenAPI` `` spec URL.
pub const SPEC_URL: &str = "https://api.prisma.io/v1/doc";

/// Provider identifier.
pub const PROVIDER_NAME: &str = "prisma_postgres";

/// Fetch the Prisma Postgres `` `OpenAPI` `` spec.
///
/// # Errors
///
/// Returns `DeploymentError` if the HTTP fetch fails or writing the file fails.
pub fn fetch_prisma_postgres_specs(
    output_dir: PathBuf,
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static,
    DeploymentError,
> {
    standard::fetch::fetch_standard_spec(PROVIDER_NAME, SPEC_URL, output_dir)
}

/// Process a fetched Prisma Postgres spec.
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
        assert_eq!(PROVIDER_NAME, "prisma_postgres");
        assert!(SPEC_URL.starts_with("https://"));
        assert!(SPEC_URL.contains("prisma.io"));
    }

    #[test]
    fn process_spec_extracts_version() {
        let spec = serde_json::json!({
            "info": { "version": "1.0.0", "title": "Prisma Postgres API" },
            "paths": {
                "/databases": {
                    "get": { "operationId": "listDatabases", "summary": "List databases" }
                }
            }
        });

        let processed = process_spec(&spec);
        assert_eq!(processed.version, Some("1.0.0".to_string()));
        assert!(processed.endpoints.is_some());
        assert!(!processed.content_hash.is_empty());
    }
}
