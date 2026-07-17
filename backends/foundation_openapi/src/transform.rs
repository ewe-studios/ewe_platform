//! Transforming a raw provider spec into a **canonical** `OpenAPI` 3.x document.
//!
//! WHY: The type and client generators expect a canonical structure — schemas in
//! `components/schemas`, a `servers` array with at least one entry, and `$ref`
//! pointers instead of inline schemas. Not all provider specs arrive that way,
//! and the gap is not cosmetic: the extractor derives a response's type name from
//! its `ref_path`, so a **fully bundled** spec (every schema inlined) yields no
//! named types at all. Measured 2026-07-17: Linode (9.3 MB) and Hetzner (3.4 MB)
//! have **zero** `$ref`s; DigitalOcean has 7814.
//!
//! WHAT: One function per transform, plus [`validate_canonical`] to check the
//! result actually is canonical rather than hoping.
//!
//! HOW: A provider composes the transforms its spec's quirks need, then validates.
//! `genapi normalize <provider>` drives this: raw → canonical artefact.
//!
//! This lived in `foundation_deployment::providers::standard::normalize` — a
//! *runtime* crate — where it was tested and called by nothing. Canonicalising a
//! spec is a build-time concern, and this is the spec-processing library
//! (spec-56 decision 05).

use serde_json::{Map, Value};

/// Ensure the spec has a `servers` array with at least one entry.
///
/// If `servers` is missing or empty, inserts `[{"url": base_url}]`.
pub fn ensure_servers(spec: &mut Value, base_url: &str) {
    let needs_server = spec
        .get("servers")
        .and_then(|s| s.as_array())
        .is_none_or(std::vec::Vec::is_empty);

    if needs_server {
        spec["servers"] = serde_json::json!([{"url": base_url}]);
    }
}

/// Convert `OpenAPI` 3.1.0 nullable type arrays to simple types with a serde default.
///
/// `OpenAPI` 3.1.0 uses `"type": ["string", "null"]` for nullable fields.
/// This normalizes them to `"type": "string"` so downstream Rust code
/// generation can map them to `Option<T>`.
pub fn normalize_nullable_types(schema: &mut Value) {
    if let Some(obj) = schema.as_object_mut() {
        if let Some(type_val) = obj.get("type") {
            if let Some(arr) = type_val.as_array() {
                let non_null: Vec<&Value> =
                    arr.iter().filter(|v| v.as_str() != Some("null")).collect();
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
///
/// # Panics
///
/// Panics if a key collected from the map is no longer present when accessed
/// (which should never happen in normal usage since we iterate over a snapshot of keys).
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
            if let Some(inner_props) = nested.get_mut("properties").and_then(|p| p.as_object_mut())
            {
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
                if let Some(inner_props) = items_schema
                    .get_mut("properties")
                    .and_then(|p| p.as_object_mut())
                {
                    extract_inline_schemas(inner_props, &item_name, schemas);
                }
                schemas.insert(item_name.clone(), items_schema);
                prop["items"] =
                    serde_json::json!({"$ref": format!("#/components/schemas/{item_name}")});
            }
        }

        // Recurse into existing properties (non-extracted objects)
        if let Some(inner_props) = prop.get_mut("properties").and_then(|p| p.as_object_mut()) {
            extract_inline_schemas(inner_props, &nested_name, schemas);
        }
    }
}

/// Convert a path like `/v1/databases/{databaseId}/backups` to a `PascalCase`
/// type name prefix like `DatabasesBackups`.
///
/// Strips version prefix (`v1`, `v2`, etc.), removes path parameters,
/// and capitalizes each segment.
#[must_use]
pub fn path_to_type_name(path: &str) -> String {
    path.split('/')
        .filter(|s| {
            !s.is_empty() && !s.starts_with('{') && !s.starts_with("v1") && !s.starts_with("v2")
        })
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

// ── validation ───────────────────────────────────────────────────────────────

/// One way a spec falls short of canonical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotCanonical {
    /// No `servers` entry — generated clients would have no base URL.
    NoServers,
    /// `components/schemas` is missing or empty, so nothing can be `$ref`'d.
    NoComponentSchemas,
    /// An operation still carries an inline object schema. The extractor names a
    /// type from its `$ref`, so this one generates as an anonymous blob.
    InlineSchema {
        /// Path the operation sits at.
        path: String,
        /// HTTP method, uppercased.
        method: String,
        /// Where in the operation — `"requestBody"` or a response status.
        location: String,
    },
}

impl std::fmt::Display for NotCanonical {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoServers => write!(f, "no `servers` entry — clients would have no base URL"),
            Self::NoComponentSchemas => {
                write!(f, "`components/schemas` is missing or empty — nothing to $ref")
            }
            Self::InlineSchema { path, method, location } => write!(
                f,
                "{method} {path} ({location}) still has an inline object schema — \
                 it would generate as an anonymous blob, since types are named from $ref"
            ),
        }
    }
}

/// Check that `spec` is canonical: `servers` present, `components/schemas`
/// populated, and no inline object schemas left in operations.
///
/// WHY: a half-applied transform produces a spec that generates *quietly wrong*
/// code — the failure mode that leaves a crate "code present" and broken. Running
/// transforms and hoping is how that happens; this is the cheap proof they worked.
///
/// Returns every problem found, not just the first — one pass should tell you
/// everything to fix.
///
/// # Errors
/// Returns the list of [`NotCanonical`] findings; empty means canonical.
pub fn validate_canonical(spec: &Value) -> Result<(), Vec<NotCanonical>> {
    let mut issues = Vec::new();

    if spec
        .get("servers")
        .and_then(Value::as_array)
        .is_none_or(|a| a.is_empty())
    {
        issues.push(NotCanonical::NoServers);
    }

    if spec
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .is_none_or(|m| m.is_empty())
    {
        issues.push(NotCanonical::NoComponentSchemas);
    }

    if let Some(paths) = spec.get("paths").and_then(Value::as_object) {
        for (path, item) in paths {
            let Some(item) = item.as_object() else { continue };
            for (method, op) in item {
                if !is_http_method(method) {
                    continue;
                }
                let Some(op) = op.as_object() else { continue };

                if let Some(schema) = op
                    .get("requestBody")
                    .and_then(|b| b.get("content"))
                    .and_then(Value::as_object)
                    .and_then(first_media_schema)
                {
                    if is_inline_object(schema) {
                        issues.push(NotCanonical::InlineSchema {
                            path: path.clone(),
                            method: method.to_uppercase(),
                            location: "requestBody".to_string(),
                        });
                    }
                }

                if let Some(responses) = op.get("responses").and_then(Value::as_object) {
                    for (status, response) in responses {
                        let Some(schema) = response
                            .get("content")
                            .and_then(Value::as_object)
                            .and_then(first_media_schema)
                        else {
                            continue;
                        };
                        if is_inline_object(schema) {
                            issues.push(NotCanonical::InlineSchema {
                                path: path.clone(),
                                method: method.to_uppercase(),
                                location: status.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

fn is_http_method(s: &str) -> bool {
    matches!(
        s.to_ascii_lowercase().as_str(),
        "get" | "post" | "put" | "patch" | "delete" | "options" | "head" | "trace"
    )
}

/// The schema of a content map's first media type (`application/json`, …).
fn first_media_schema(content: &Map<String, Value>) -> Option<&Value> {
    content.values().find_map(|m| m.get("schema"))
}

/// An object schema written out in place rather than referenced.
///
/// A `$ref` is fine, and so is a bare `{"type": "object"}` with no properties
/// (a free-form map) — it has no fields to name a type from. What is not fine is
/// an object with `properties`, which is a struct the generator cannot name.
fn is_inline_object(schema: &Value) -> bool {
    if schema.get("$ref").is_some() {
        return false;
    }
    if schema.get("properties").is_some() || schema.get("allOf").is_some() {
        return true;
    }
    // An array whose items are an inline object has the same problem.
    if schema.get("type").and_then(Value::as_str) == Some("array") {
        if let Some(items) = schema.get("items") {
            return is_inline_object(items);
        }
    }
    false
}
