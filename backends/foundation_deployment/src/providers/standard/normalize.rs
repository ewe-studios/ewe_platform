//! Shared spec normalization utilities for OpenAPI 3.x providers.
//!
//! WHY: The type and client generators expect a canonical OpenAPI 3.x structure:
//! schemas in `components/schemas`, a `servers` array with at least one entry,
//! and `$ref` pointers instead of inline schemas. Not all provider specs arrive
//! in this form.
//!
//! WHAT: Functions to normalize raw OpenAPI specs into the canonical form.
//!
//! HOW: Each function transforms one aspect of the spec. Providers compose them
//! in their `fetch.rs` to build a normalizer that matches their spec's quirks.

use serde_json::{Map, Value};

/// Ensure the spec has a `servers` array with at least one entry.
///
/// If `servers` is missing or empty, inserts `[{"url": base_url}]`.
pub fn ensure_servers(spec: &mut Value, base_url: &str) {
    let needs_server = spec
        .get("servers")
        .and_then(|s| s.as_array())
        .map_or(true, |a| a.is_empty());

    if needs_server {
        spec["servers"] = serde_json::json!([{"url": base_url}]);
    }
}

/// Convert OpenAPI 3.1.0 nullable type arrays to simple types with a serde default.
///
/// OpenAPI 3.1.0 uses `"type": ["string", "null"]` for nullable fields.
/// This normalizes them to `"type": "string"` so downstream Rust code
/// generation can map them to `Option<T>`.
pub fn normalize_nullable_types(schema: &mut Value) {
    if let Some(obj) = schema.as_object_mut() {
        if let Some(type_val) = obj.get("type") {
            if let Some(arr) = type_val.as_array() {
                let non_null: Vec<&Value> = arr.iter().filter(|v| v.as_str() != Some("null")).collect();
                if non_null.len() == 1 {
                    let actual_type = non_null[0].clone();
                    obj.insert("type".to_string(), actual_type);
                    obj.insert("nullable".to_string(), Value::Bool(true));
                }
            }
        }
        // Recurse into properties
        if let Some(props) = obj.get_mut("properties") {
            if let Some(props_obj) = props.as_object_mut() {
                for (_key, prop_val) in props_obj.iter_mut() {
                    normalize_nullable_types(prop_val);
                }
            }
        }
        // Recurse into items
        if let Some(items) = obj.get_mut("items") {
            normalize_nullable_types(items);
        }
    }
}

/// Extract inline object schemas from a property map into `components/schemas`.
///
/// Walks `properties` recursively. When it finds a property with `"type": "object"`
/// and inline `properties`, it extracts it to `schemas` under `{parent_name}{PropName}`
/// and replaces the inline definition with a `$ref`.
///
/// For array properties whose `items` contain an inline object, extracts similarly.
pub fn extract_inline_schemas(
    properties: &mut Map<String, Value>,
    parent_name: &str,
    schemas: &mut Map<String, Value>,
) {
    let keys: Vec<String> = properties.keys().cloned().collect();
    for key in keys {
        let prop = properties.get_mut(&key).unwrap();
        let nested_name = format!("{}{}{}", parent_name, key[..1].to_uppercase(), &key[1..]);

        // Nested object with inline properties
        let is_inline_object = prop.get("properties").is_some()
            && (prop.get("type").and_then(|t| t.as_str()) == Some("object")
                || prop.get("type").is_none());

        if is_inline_object {
            let mut nested = prop.take();
            normalize_nullable_types(&mut nested);
            if nested.get("type").is_none() {
                nested["type"] = Value::String("object".to_string());
            }
            // Recurse into nested properties
            if let Some(inner_props) = nested.get_mut("properties").and_then(|p| p.as_object_mut()) {
                extract_inline_schemas(inner_props, &nested_name, schemas);
            }
            schemas.insert(nested_name.clone(), nested);
            *prop = serde_json::json!({"$ref": format!("#/components/schemas/{nested_name}")});
            continue;
        }

        // Array of inline objects
        if prop.get("type").and_then(|t| t.as_str()) == Some("array") {
            let items_has_props = prop
                .get("items")
                .and_then(|i| i.get("properties"))
                .is_some();
            if items_has_props {
                let item_name = format!("{nested_name}Item");
                let mut items_schema = prop["items"].take();
                normalize_nullable_types(&mut items_schema);
                if items_schema.get("type").is_none() {
                    items_schema["type"] = Value::String("object".to_string());
                }
                if let Some(inner_props) = items_schema.get_mut("properties").and_then(|p| p.as_object_mut()) {
                    extract_inline_schemas(inner_props, &item_name, schemas);
                }
                schemas.insert(item_name.clone(), items_schema);
                prop["items"] = serde_json::json!({"$ref": format!("#/components/schemas/{item_name}")});
            }
        }

        // Recurse into existing properties (non-extracted objects)
        if let Some(inner_props) = prop.get_mut("properties").and_then(|p| p.as_object_mut()) {
            extract_inline_schemas(inner_props, &nested_name, schemas);
        }
    }
}

/// Convert a path like `/v1/databases/{databaseId}/backups` to a PascalCase
/// type name prefix like `DatabasesBackups`.
///
/// Strips version prefix (`v1`, `v2`, etc.), removes path parameters,
/// and capitalizes each segment.
pub fn path_to_type_name(path: &str) -> String {
    path.split('/')
        .filter(|s| !s.is_empty() && !s.starts_with('{') && !s.starts_with("v1") && !s.starts_with("v2"))
        .map(|s| {
            let cleaned = s.replace('-', "_");
            let mut chars = cleaned.chars();
            match chars.next() {
                Some(c) => {
                    let mut result = c.to_uppercase().to_string();
                    result.extend(chars);
                    result
                }
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(path_to_type_name("/v1/databases/{databaseId}/backups"), "DatabasesBackups");
        assert_eq!(path_to_type_name("/v1/compute-services/{id}/versions"), "Compute_servicesVersions");
    }

    #[test]
    fn extract_inline_schemas_extracts_nested_object() {
        let mut props = serde_json::Map::new();
        props.insert("project".to_string(), json!({
            "type": "object",
            "properties": {
                "id": {"type": "string"},
                "name": {"type": "string"}
            }
        }));
        let mut schemas = serde_json::Map::new();
        extract_inline_schemas(&mut props, "Database", &mut schemas);

        assert!(props["project"].get("$ref").is_some());
        assert!(schemas.contains_key("DatabaseProject"));
        assert_eq!(schemas["DatabaseProject"]["properties"]["id"]["type"], "string");
    }

    #[test]
    fn extract_inline_schemas_extracts_array_items() {
        let mut props = serde_json::Map::new();
        props.insert("items".to_string(), json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "id": {"type": "string"}
                }
            }
        }));
        let mut schemas = serde_json::Map::new();
        extract_inline_schemas(&mut props, "List", &mut schemas);

        assert!(schemas.contains_key("ListItemsItem"));
    }
}
