//! Shared OpenAPI 3.x extraction utilities.
//!
//! WHY: All standard providers use OpenAPI 3.0 format with the same structure
//! for version info and endpoint paths. Centralizing extraction avoids
//! duplicating identical logic across provider modules.
//!
//! WHAT: Functions to extract version strings, API endpoints, and content
//! hashes from parsed OpenAPI JSON specs.
//!
//! HOW: Reads `info.version` for versions, iterates `paths` for endpoints,
//! and hashes the serialized JSON for change detection.

use serde::{Deserialize, Serialize};

/// A single API endpoint extracted from an OpenAPI spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiEndpoint {
    /// URL path (e.g. "/v1/projects").
    pub path: String,
    /// HTTP methods (e.g. \["GET"\]).
    pub methods: Vec<String>,
    /// Operation identifier from the spec.
    pub operation_id: Option<String>,
    /// Human-readable summary.
    pub summary: Option<String>,
}

/// Processed result from an OpenAPI spec.
#[derive(Debug, Clone)]
pub struct ProcessedSpec {
    /// API version extracted from `info.version`.
    pub version: Option<String>,
    /// Extracted endpoints from `paths`.
    pub endpoints: Option<Vec<ApiEndpoint>>,
    /// Content hash for change detection.
    pub content_hash: String,
}

/// Extract the API version from an OpenAPI spec's `info.version` field.
pub fn extract_version(spec: &serde_json::Value) -> Option<String> {
    spec.get("info")
        .and_then(|info| info.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Extract API endpoints from an OpenAPI spec's `paths` object.
///
/// Each HTTP method on each path becomes a separate `ApiEndpoint`.
pub fn extract_endpoints(spec: &serde_json::Value) -> Option<Vec<ApiEndpoint>> {
    let paths_obj = spec.get("paths").and_then(|p| p.as_object())?;

    let endpoints: Vec<ApiEndpoint> = paths_obj
        .iter()
        .flat_map(|(path, path_item)| {
            let methods = path_item.as_object();
            methods
                .into_iter()
                .flat_map(|obj| {
                    ["get", "post", "put", "patch", "delete"]
                        .iter()
                        .filter_map(|method| {
                            let operation = obj.get(*method)?;
                            Some(ApiEndpoint {
                                path: path.clone(),
                                methods: vec![method.to_uppercase()],
                                operation_id: operation
                                    .get("operationId")
                                    .and_then(|v| v.as_str())
                                    .map(String::from),
                                summary: operation
                                    .get("summary")
                                    .and_then(|v| v.as_str())
                                    .map(String::from),
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect();

    if endpoints.is_empty() {
        None
    } else {
        Some(endpoints)
    }
}

/// Compute a content hash for change detection.
///
/// Uses `DefaultHasher` for speed — not cryptographic, just for detecting
/// whether a spec has changed between fetches.
pub fn compute_content_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Process an OpenAPI spec JSON into version, endpoints, and hash.
pub fn process_spec(spec: &serde_json::Value) -> ProcessedSpec {
    let version = extract_version(spec);
    let endpoints = extract_endpoints(spec);
    let content_hash = compute_content_hash(&spec.to_string());
    ProcessedSpec {
        version,
        endpoints,
        content_hash,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_openapi_spec() -> serde_json::Value {
        serde_json::json!({
            "openapi": "3.0.0",
            "info": {
                "title": "Test API",
                "version": "2.1.0"
            },
            "paths": {
                "/projects": {
                    "get": {
                        "operationId": "listProjects",
                        "summary": "List all projects"
                    },
                    "post": {
                        "operationId": "createProject",
                        "summary": "Create a project"
                    }
                },
                "/projects/{id}": {
                    "get": {
                        "operationId": "getProject",
                        "summary": "Get a project"
                    },
                    "delete": {
                        "operationId": "deleteProject",
                        "summary": "Delete a project"
                    }
                }
            }
        })
    }

    #[test]
    fn extracts_version_from_info() {
        let spec = sample_openapi_spec();
        assert_eq!(extract_version(&spec), Some("2.1.0".to_string()));
    }

    #[test]
    fn returns_none_for_missing_version() {
        let spec = serde_json::json!({"info": {}});
        assert_eq!(extract_version(&spec), None);
    }

    #[test]
    fn returns_none_for_missing_info() {
        let spec = serde_json::json!({});
        assert_eq!(extract_version(&spec), None);
    }

    #[test]
    fn extracts_endpoints_from_paths() {
        let spec = sample_openapi_spec();
        let endpoints = extract_endpoints(&spec).expect("should extract endpoints");
        assert_eq!(endpoints.len(), 4);

        let list = endpoints
            .iter()
            .find(|e| e.operation_id.as_deref() == Some("listProjects"))
            .expect("listProjects endpoint");
        assert_eq!(list.path, "/projects");
        assert_eq!(list.methods, vec!["GET"]);
        assert_eq!(list.summary.as_deref(), Some("List all projects"));
    }

    #[test]
    fn returns_none_for_empty_paths() {
        let spec = serde_json::json!({"paths": {}});
        assert_eq!(extract_endpoints(&spec), None);
    }

    #[test]
    fn returns_none_for_missing_paths() {
        let spec = serde_json::json!({});
        assert_eq!(extract_endpoints(&spec), None);
    }

    #[test]
    fn content_hash_is_deterministic() {
        let content = r#"{"key": "value"}"#;
        let hash1 = compute_content_hash(content);
        let hash2 = compute_content_hash(content);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 16);
    }

    #[test]
    fn content_hash_differs_for_different_content() {
        let hash1 = compute_content_hash("content A");
        let hash2 = compute_content_hash("content B");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn process_spec_returns_all_fields() {
        let spec = sample_openapi_spec();
        let processed = process_spec(&spec);
        assert_eq!(processed.version, Some("2.1.0".to_string()));
        assert_eq!(processed.endpoints.as_ref().map(Vec::len), Some(4));
        assert!(!processed.content_hash.is_empty());
    }
}
