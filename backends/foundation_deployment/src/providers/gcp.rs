//! GCP Discovery Service spec fetcher.
//!
//! Extracts endpoints from GCP's Discovery API directory format.

use serde_json::Value;

/// Endpoint extracted from GCP Discovery spec.
#[derive(Debug, Clone)]
pub struct GcpEndpoint {
    pub path: String,
    pub methods: Vec<String>,
    pub operation_id: Option<String>,
    pub summary: Option<String>,
}

/// Extract endpoints from GCP Discovery API directory response.
///
/// GCP returns a directory of APIs in the format:
/// ```json
/// {
///   "kind": "discovery#directoryList",
///   "items": [
///     {
///       "id": "run:v2",
///       "name": "run",
///       "version": "v2",
///       "title": "Cloud Run Admin API",
///       "discoveryRestUrl": "https://run.googleapis.com/$discovery/rest?version=v2"
///     }
///   ]
/// }
/// ```
pub fn extract_endpoints(spec: &Value) -> Option<Vec<GcpEndpoint>> {
    spec.get("items")
        .and_then(|items| items.as_array())
        .map(|apis| {
            apis.iter()
                .filter_map(|api| {
                    Some(GcpEndpoint {
                        path: api.get("name")?.as_str()?.to_string(),
                        methods: vec!["GET".to_string()],
                        operation_id: api.get("id").and_then(|v| v.as_str()).map(String::from),
                        summary: api.get("title").and_then(|v| v.as_str()).map(String::from),
                    })
                })
                .collect()
        })
}

/// Extract version from GCP spec.
pub fn extract_version(_spec: &Value) -> Option<String> {
    // GCP directory doesn't have a single version - use timestamp
    None
}
