//! Stripe OpenAPI spec fetcher.
//!
//! WHY: Stripe is a payment processing platform whose API is critical for
//! applications that handle billing and subscriptions.
//!
//! WHAT: Fetches the Stripe OpenAPI 3.0 spec from GitHub and writes it to
//! the provider's output directory.
//!
//! HOW: Delegates to `standard::fetch::fetch_standard_spec` for HTTP download.
//! Provides `process_spec` for post-fetch extraction. Stripe's spec is
//! notably large (~10MB+) with hundreds of endpoints across resource types.

use crate::error::DeploymentError;
use crate::providers::openapi::{self, ProcessedSpec};
use crate::providers::standard;
use foundation_core::valtron::StreamIterator;
use std::path::PathBuf;

/// Stripe OpenAPI 3.0 spec URL (from GitHub raw).
pub const SPEC_URL: &str =
    "https://raw.githubusercontent.com/stripe/openapi/master/openapi/spec3.json";

/// Provider identifier.
pub const PROVIDER_NAME: &str = "stripe";

/// Fetch the Stripe OpenAPI spec.
pub fn fetch_stripe_specs(
    output_dir: PathBuf,
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = ()> + Send + 'static,
    DeploymentError,
> {
    standard::fetch::fetch_standard_spec(PROVIDER_NAME, SPEC_URL, output_dir)
}

/// Process a fetched Stripe spec.
///
/// Stripe's spec is large (~10MB+), so the content hash computation
/// may take longer than other providers.
pub fn process_spec(spec: &serde_json::Value) -> ProcessedSpec {
    openapi::process_spec(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_correct() {
        assert_eq!(PROVIDER_NAME, "stripe");
        assert!(SPEC_URL.starts_with("https://"));
        assert!(SPEC_URL.contains("stripe"));
    }

    #[test]
    fn process_spec_extracts_payment_endpoints() {
        let spec = serde_json::json!({
            "info": { "version": "2023-10-16", "title": "Stripe API" },
            "paths": {
                "/v1/charges": {
                    "get": { "operationId": "GetCharges", "summary": "List charges" },
                    "post": { "operationId": "PostCharges", "summary": "Create a charge" }
                },
                "/v1/customers": {
                    "get": { "operationId": "GetCustomers", "summary": "List customers" },
                    "post": { "operationId": "PostCustomers", "summary": "Create a customer" }
                },
                "/v1/payment_intents": {
                    "get": { "operationId": "GetPaymentIntents", "summary": "List payment intents" },
                    "post": { "operationId": "PostPaymentIntents", "summary": "Create a payment intent" }
                }
            }
        });

        let processed = process_spec(&spec);
        assert_eq!(processed.version, Some("2023-10-16".to_string()));
        assert_eq!(processed.endpoints.as_ref().map(Vec::len), Some(6));
    }
}
