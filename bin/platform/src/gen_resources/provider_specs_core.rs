//! Core types for provider spec fetching.

use serde::{Deserialize, Serialize};

/// WHY: Tracks progress of individual spec fetches during parallel execution.
///
/// WHAT: Progress states with source identification for observability.
///
/// HOW: Used as the `Pending` type in `TaskIterator` combinators.
#[derive(Debug, Clone)]
pub enum SpecFetchPending {
    Connecting { provider: &'static str },
    AwaitingResponse { provider: &'static str },
    Parsing { provider: &'static str },
}

impl std::fmt::Display for SpecFetchPending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecFetchPending::Connecting { provider } => {
                write!(f, "{provider}: Connecting...")
            }
            SpecFetchPending::AwaitingResponse { provider } => {
                write!(f, "{provider}: Awaiting response...")
            }
            SpecFetchPending::Parsing { provider } => {
                write!(f, "{provider}: Parsing JSON...")
            }
        }
    }
}

/// WHY: Unified representation of API metadata from providers.
///
/// WHAT: Common metadata extracted from any provider's spec format.
///
/// HOW: Normalized format that all providers can be converted to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct APIMetadata {
    /// Provider name (e.g., "fly-io", "gcp", "neon")
    pub provider: String,

    /// Spec version or timestamp
    pub version: String,

    /// Fetched at timestamp
    pub fetched_at: chrono::DateTime<chrono::Utc>,

    /// Source URL
    pub source_url: String,

    /// Raw OpenAPI spec (preserved as-is)
    pub raw_spec: serde_json::Value,

    /// Extracted API endpoints (optional, provider-specific)
    pub endpoints: Option<Vec<SpecEndpoint>>,

    /// Change detection hash
    pub content_hash: String,

    /// List of written spec file paths (relative to artefacts dir).
    /// Single-spec providers have one entry; multi-spec providers (e.g., GCP) have multiple.
    pub spec_files: Vec<String>,
}

/// WHY: Represents a single API endpoint extracted from a spec.
///
/// WHAT: Normalized endpoint info for cross-provider comparisons.
///
/// HOW: Extracted from OpenAPI paths during distillation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpecEndpoint {
    pub path: String,
    pub methods: Vec<String>,
    pub operation_id: Option<String>,
    pub summary: Option<String>,
}

/// WHY: Result of a spec fetch operation.
///
/// WHAT: Contains the API metadata and metadata about the fetch.
///
/// HOW: Returned by individual fetch tasks.
pub type FetchResult = Result<APIMetadata, crate::gen_resources::provider_specs_errors::SpecFetchError>;
