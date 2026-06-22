use foundation_deployment::providers::standard::normalize::{
    ensure_servers, extract_inline_schemas, normalize_nullable_types, path_to_type_name,
};
use serde_json::json;

#[test]
fn ensure_servers_adds_when_missing() {
    let mut spec = json!({"openapi": "3.1.0"});
    ensure_servers(&mut spec, "https://api.example.com");
    assert_eq!(spec["servers"][0]["url"], "https://api.example.com");
}

#[test]
fn ensure_servers_adds_when_empty() {
    let mut spec = json!({"servers": []});
    ensure_servers(&mut spec, "https://api.example.com");
    assert_eq!(spec["servers"][0]["url"], "https://api.example.com");
}

#[test]
fn ensure_servers_preserves_existing() {
    let mut spec = json!({"servers": [{"url": "https://existing.com"}]});
    ensure_servers(&mut spec, "https://api.example.com");
    assert_eq!(spec["servers"][0]["url"], "https://existing.com");
}

#[test]
fn normalize_nullable_converts_type_array() {
    let mut schema = json!({"type": ["string", "null"]});
    normalize_nullable_types(&mut schema);
    assert_eq!(schema["type"], "string");
    assert_eq!(schema["nullable"], true);
}

#[test]
fn normalize_nullable_leaves_single_type() {
    let mut schema = json!({"type": "string"});
    normalize_nullable_types(&mut schema);
    assert_eq!(schema["type"], "string");
    assert!(schema.get("nullable").is_none());
}

#[test]
fn path_to_type_name_strips_params_and_version() {
    assert_eq!(path_to_type_name("/v1/databases"), "Databases");
    assert_eq!(
        path_to_type_name("/v1/databases/{databaseId}/backups"),
        "DatabasesBackups"
    );
    assert_eq!(
        path_to_type_name("/v1/compute-services/{id}/versions"),
        "Compute_servicesVersions"
    );
}

#[test]
fn extract_inline_schemas_extracts_nested_object() {
    let mut props = serde_json::Map::new();
    props.insert(
        "project".to_string(),
        json!({
            "type": "object",
            "properties": {
                "id": {"type": "string"},
                "name": {"type": "string"}
            }
        }),
    );
    let mut schemas = serde_json::Map::new();
    extract_inline_schemas(&mut props, "Database", &mut schemas);

    assert!(props["project"].get("$ref").is_some());
    assert!(schemas.contains_key("DatabaseProject"));
    assert_eq!(
        schemas["DatabaseProject"]["properties"]["id"]["type"],
        "string"
    );
}

#[test]
fn extract_inline_schemas_extracts_array_items() {
    let mut props = serde_json::Map::new();
    props.insert(
        "items".to_string(),
        json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "id": {"type": "string"}
                }
            }
        }),
    );
    let mut schemas = serde_json::Map::new();
    extract_inline_schemas(&mut props, "List", &mut schemas);

    assert!(schemas.contains_key("ListItemsItem"));
}
