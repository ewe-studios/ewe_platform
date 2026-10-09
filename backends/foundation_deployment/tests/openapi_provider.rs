use foundation_deployment::providers::openapi::{
    compute_content_hash, extract_endpoints, extract_version, process_spec,
};

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
