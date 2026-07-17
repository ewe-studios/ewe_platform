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

// ── canonicalisation ─────────────────────────────────────────────────────────

/// What [`canonicalize_operations`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanonicalizeStats {
    /// Inline operation schemas hoisted into `components/schemas`.
    pub hoisted: usize,
    /// Names that collided and were suffixed to stay unique.
    pub renamed: usize,
}

/// Hoist every inline operation schema into `components/schemas`, leaving a
/// `$ref` behind.
///
/// WHY: the extractor names a response's type from its `ref_path`, so an inline
/// schema generates as an anonymous blob (`serde_json::Value`). Measured
/// 2026-07-17: Linode has 1042 inline operation schemas, Hetzner 633 — and even
/// Cloudflare, which is generated today, has 3090 (1224 of its 2442 generated
/// request fns return `serde_json::Value` or `()`).
///
/// WHAT: request bodies and every response, per operation. Nested inline objects
/// inside a hoisted schema are handled by [`extract_inline_schemas`].
///
/// HOW: names come from the operation's `operationId` (unique per operation by
/// definition) — `post-linode-instance` → `PostLinodeInstanceRequest` /
/// `PostLinodeInstanceResponse`. Without one, the path and method are used;
/// `path_to_type_name` alone would collide, since it strips parameters and would
/// give `/instances` and `/instances/{id}` the same name.
pub fn canonicalize_operations(spec: &mut Value) -> CanonicalizeStats {
    let mut stats = CanonicalizeStats::default();
    let mut hoisted: Map<String, Value> = Map::new();

    let Some(paths) = spec.get_mut("paths").and_then(Value::as_object_mut) else {
        return stats;
    };

    for (path, item) in paths.iter_mut() {
        let Some(item) = item.as_object_mut() else { continue };
        for (method, op) in item.iter_mut() {
            if !is_http_method(method) {
                continue;
            }
            let Some(op) = op.as_object_mut() else { continue };

            let base = operation_type_base(op, path, method);

            if let Some(schema) = op
                .get_mut("requestBody")
                .and_then(|b| b.get_mut("content"))
                .and_then(Value::as_object_mut)
                .and_then(first_media_schema_mut)
            {
                hoist(schema, &format!("{base}Request"), &mut hoisted, &mut stats);
            }

            if let Some(responses) = op.get_mut("responses").and_then(Value::as_object_mut) {
                // Deterministic order, and the first success gets the plain name.
                let statuses: Vec<String> = responses.keys().cloned().collect();
                let mut plain_taken = false;
                for status in statuses {
                    let is_success = status.starts_with('2');
                    let name = if is_success && !plain_taken {
                        plain_taken = true;
                        format!("{base}Response")
                    } else {
                        format!("{base}{status}Response")
                    };
                    if let Some(schema) = responses
                        .get_mut(&status)
                        .and_then(|r| r.get_mut("content"))
                        .and_then(Value::as_object_mut)
                        .and_then(first_media_schema_mut)
                    {
                        hoist(schema, &name, &mut hoisted, &mut stats);
                    }
                }
            }
        }
    }

    if !hoisted.is_empty() {
        let schemas = spec
            .as_object_mut()
            .expect("spec is an object")
            .entry("components")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .expect("components is an object")
            .entry("schemas")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .expect("schemas is an object");
        for (name, schema) in hoisted {
            schemas.insert(name, schema);
        }
    }

    stats
}

/// Replace `schema` with a `$ref` to `name`, parking the schema itself in
/// `hoisted` — recursing into its properties so nested objects get names too.
fn hoist(schema: &mut Value, name: &str, hoisted: &mut Map<String, Value>, stats: &mut CanonicalizeStats) {
    if !is_inline_object(schema) {
        return; // already a $ref, or free-form — nothing to name
    }

    // An array of inline objects: name the *item*, keep the array inline.
    if schema.get("type").and_then(Value::as_str) == Some("array") {
        if let Some(items) = schema.get_mut("items") {
            hoist(items, &format!("{name}Item"), hoisted, stats);
        }
        return;
    }

    let mut taken = schema.take();
    normalize_nullable_types(&mut taken);
    if taken.get("type").is_none() {
        taken["type"] = Value::String("object".to_string());
    }
    if let Some(props) = taken.get_mut("properties").and_then(Value::as_object_mut) {
        extract_inline_schemas(props, name, hoisted);
    }

    let final_name = unique_name(name, &taken, hoisted, stats);
    hoisted.insert(final_name.clone(), taken);
    *schema = serde_json::json!({ "$ref": format!("#/components/schemas/{final_name}") });
    stats.hoisted += 1;
}

/// A name nothing else has taken — unless the taker is byte-identical, in which
/// case sharing it is correct (the same shape twice is one type).
fn unique_name(
    name: &str,
    schema: &Value,
    hoisted: &Map<String, Value>,
    stats: &mut CanonicalizeStats,
) -> String {
    match hoisted.get(name) {
        None => name.to_string(),
        Some(existing) if existing == schema => name.to_string(),
        Some(_) => {
            for n in 2..1000 {
                let candidate = format!("{name}{n}");
                match hoisted.get(&candidate) {
                    None => {
                        stats.renamed += 1;
                        return candidate;
                    }
                    Some(existing) if existing == schema => return candidate,
                    Some(_) => {}
                }
            }
            stats.renamed += 1;
            format!("{name}Dedup")
        }
    }
}

/// The type-name stem for an operation.
fn operation_type_base(op: &Map<String, Value>, path: &str, method: &str) -> String {
    if let Some(id) = op.get("operationId").and_then(Value::as_str) {
        if !id.is_empty() {
            return crate::api_catalog::to_pascal_case_from_any(id);
        }
    }
    // No operationId: path alone collides (it strips parameters), so add the method.
    let mut base = path_to_type_name(path);
    let lowered = method.to_lowercase();
    let mut chars = lowered.chars();
    if let Some(c) = chars.next() {
        base.push_str(&c.to_uppercase().to_string());
        base.extend(chars);
    }
    base
}

fn first_media_schema_mut(content: &mut Map<String, Value>) -> Option<&mut Value> {
    content.values_mut().find_map(|m| m.get_mut("schema"))
}
