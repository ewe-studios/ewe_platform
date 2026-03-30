//! Stripe API spec fetcher.
//!
//! Extracts endpoints from Stripe's OpenAPI spec format.

use serde_json::Value;

/// Endpoint extracted from Stripe spec.
#[derive(Debug, Clone)]
pub struct StripeEndpoint {
    pub path: String,
    pub methods: Vec<String>,
    pub operation_id: Option<String>,
    pub summary: Option<String>,
}

/// Extract endpoints from Stripe OpenAPI spec.
///
/// Stripe uses a standard OpenAPI 3.x format with paths:
/// ```json
/// {
///   "openapi": "3.0.0",
///   "paths": {
///     "/v1/charges": { ... },
///     "/v1/customers": { ... }
///   }
/// }
/// ```
pub fn extract_endpoints(spec: &Value) -> Option<Vec<StripeEndpoint>> {
    spec.get("paths")
        .and_then(|paths| paths.as_object())
        .map(|paths_obj| {
            paths_obj
                .keys()
                .map(|path| StripeEndpoint {
                    path: path.clone(),
                    methods: vec![],
                    operation_id: None,
                    summary: None,
                })
                .collect()
        })
}

/// Extract version from Stripe spec.
pub fn extract_version(spec: &Value) -> Option<String> {
    spec.get("info")
        .and_then(|i| i.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from)
}
