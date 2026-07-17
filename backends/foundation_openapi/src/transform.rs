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

use std::collections::BTreeSet;

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

/// A property key, turned into a name fragment that is safe to concatenate.
///
/// **Why this is not `key[..1].to_uppercase() + &key[1..]`:** a property key is
/// arbitrary text, and a component's name is a JSON-pointer segment. Cloudflare
/// has properties keyed `application/json` — pasted in raw, that yields
/// `…ResponseContentApplication/json`, whose `$ref` **cannot resolve**: the `/`
/// reads as a pointer separator, so the schema is written but every reference to
/// it dangles. 61 of them, caught by `dangling_refs` on Cloudflare's real spec.
///
/// Uppercasing byte 0 also panics outright on a key whose first character is
/// multi-byte.
fn nested_suffix(key: &str) -> String {
    crate::api_catalog::to_pascal_case_from_any(key)
}

/// Extract inline object schemas from a property map into `components/schemas`.
///
/// Walks `properties` recursively. When it finds a property with `"type": "object"`
/// and inline `properties`, it extracts it to `schemas` under `{parent_name}{PropName}`
/// and replaces the inline definition with a `$ref`.
///
/// For array properties whose `items` contain an inline object, extracts similarly.
///
/// Names go through [`nested_suffix`], so a property keyed with a slash, a dash or
/// a space still yields a component name that a `$ref` can resolve.
pub fn extract_inline_schemas(
    properties: &mut Map<String, Value>,
    parent_name: &str,
    schemas: &mut Map<String, Value>,
) {
    let keys: Vec<String> = properties.keys().cloned().collect();
    for key in keys {
        let prop = properties.get_mut(&key).unwrap();
        let nested_name = format!("{parent_name}{}", nested_suffix(&key));

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
    // A `oneOf`/`anyOf` is a union, and we do not generate Rust enums for them —
    // the generator emits a flattened `HashMap<String, Value>` instead. Hoisting
    // it anyway is still worth it: DigitalOcean's `droplet_create` response is a
    // `oneOf` (one droplet, or many), and left unhoisted the endpoint's return
    // type resolves to `()` and THE BODY IS DISCARDED ENTIRELY. Named-but-opaque
    // hands the caller the JSON; unnamed hands them nothing.
    if schema.get("oneOf").is_some() || schema.get("anyOf").is_some() {
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

    // Names the vendor has already spent, indexed the way the *generator* sees
    // them. This is not paranoia: Cloudflare bundles `vectorize_index_info_response`
    // and has an operation `vectorize-index-info`, whose response we would name
    // `VectorizeIndexInfoResponse` — the same Rust type. The field referencing the
    // vendor's schema then renders as its own parent, which surfaces as
    // "recursive type has infinite size" and is really two schemas fighting over
    // one name. Raw keys alone miss it: the two collide only after pascal-casing.
    let reserved: BTreeSet<String> = spec
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .map(|schemas| {
            schemas
                .keys()
                .map(|k| crate::api_catalog::to_pascal_case_from_any(k))
                .collect()
        })
        .unwrap_or_default();

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
                hoist(schema, &format!("{base}Request"), &mut hoisted, &reserved, &mut stats);
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
                        hoist(schema, &name, &mut hoisted, &reserved, &mut stats);
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
fn hoist(
    schema: &mut Value,
    name: &str,
    hoisted: &mut Map<String, Value>,
    reserved: &BTreeSet<String>,
    stats: &mut CanonicalizeStats,
) {
    if !is_inline_object(schema) {
        return; // already a $ref, or free-form — nothing to name
    }

    // An array of inline objects: name the *item*, keep the array inline.
    if schema.get("type").and_then(Value::as_str) == Some("array") {
        if let Some(items) = schema.get_mut("items") {
            hoist(items, &format!("{name}Item"), hoisted, reserved, stats);
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

    let final_name = unique_name(name, &taken, hoisted, reserved, stats);
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
    reserved: &BTreeSet<String>,
    stats: &mut CanonicalizeStats,
) -> String {
    for n in 0..1000 {
        let candidate = if n == 0 {
            name.to_string()
        } else {
            format!("{name}{}", n + 1)
        };

        // A name the vendor already owns is never ours to take — not even when
        // the schemas look identical. Two component keys that pascal-case to one
        // type name mean the generator emits the struct twice.
        if reserved.contains(&crate::api_catalog::to_pascal_case_from_any(&candidate)) {
            continue;
        }
        match hoisted.get(&candidate) {
            // Free.
            None => {
                if n > 0 {
                    stats.renamed += 1;
                }
                return candidate;
            }
            // The same shape twice is one type — sharing the name is correct.
            Some(existing) if existing == schema => return candidate,
            Some(_) => {}
        }
    }
    stats.renamed += 1;
    format!("{name}Dedup")
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

/// Fields that document an API but generate nothing.
///
/// `description` and `summary` are **not** here — they become doc comments.
const DOC_ONLY_FIELDS: &[&str] = &[
    // Schema/media-type examples.
    "example",
    "examples",
    // Vendor code samples embedded in the spec (DigitalOcean ships curl/Go/Ruby
    // snippets under this).
    "x-codeSamples",
    "x-code-samples",
];

/// Strip fields that only document the API, leaving what generates code.
///
/// WHY: two reasons, one of which is not obvious.
///
/// 1. **Weight.** They are 34% of DigitalOcean's spec — 2.3 MB → 1.6 MB — and
///    nothing in the generator reads them.
/// 2. **They carry credential-shaped strings.** DigitalOcean's spec embeds Slack
///    webhook URLs in `example` values and `x-codeSamples` snippets (11 of them).
///    They are the vendor's own documentation placeholders, not live secrets — but
///    committing the artefact trips GitHub's push protection, and a repository that
///    trains people to click "allow this secret" is worse off than one that does
///    not carry examples it never uses.
///
/// `description` and `summary` survive: they become doc comments on the generated
/// types.
pub fn strip_doc_only(spec: &mut Value) {
    match spec {
        Value::Object(map) => {
            for field in DOC_ONLY_FIELDS {
                map.remove(*field);
            }
            for value in map.values_mut() {
                strip_doc_only(value);
            }
        }
        Value::Array(items) => {
            for value in items {
                strip_doc_only(value);
            }
        }
        _ => {}
    }
}

/// What [`resolve_parameter_refs`] rewired.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParameterRefStats {
    /// Operation parameters rewritten from a `$ref` into their real definition.
    pub parameters_inlined: usize,
    /// `$ref`s naming a component that does not exist — the vendor's bug, left alone.
    pub unresolved: usize,
}

/// Resolve `$ref`s into `components/parameters` so operations carry their
/// parameters directly.
///
/// **WHY:** the twin of [`resolve_response_refs`], and it bites harder. DigitalOcean
/// writes **every** parameter as a reference —
/// `{"$ref": "#/components/parameters/droplet_tag_name"}` — and our `Parameter`
/// model has `name`/`in`/`required`/`schema` and **no `$ref` field**. So each one
/// deserialises into a nameless parameter and is dropped: **833 of DO's 1030
/// parameters vanish**, silently, and `droplets_list` generates with no arguments
/// at all.
///
/// That is not cosmetic. `?tag_name=` is how a deployment enumerates *its own*
/// droplets — DigitalOcean's answer to Hetzner's label selector — so losing it
/// takes the identity mechanism with it.
///
/// **WHAT:** replaces each `$ref` parameter with the component's body.
///
/// **HOW:** a straight substitution — unlike responses, a parameter has no schema
/// to hoist, so there is nothing to name. A `$ref` naming a component that does not
/// exist is left exactly as it is: that is the vendor's bug, and inventing an empty
/// parameter would hide it.
///
/// Run it **before** [`canonicalize_operations`].
pub fn resolve_parameter_refs(spec: &mut Value) -> ParameterRefStats {
    let mut stats = ParameterRefStats::default();

    let components: Map<String, Value> = spec
        .pointer("/components/parameters")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if components.is_empty() {
        return stats;
    }

    let Some(paths) = spec.get_mut("paths").and_then(Value::as_object_mut) else {
        return stats;
    };

    for (_, item) in paths.iter_mut() {
        let Some(item) = item.as_object_mut() else { continue };
        for (method, op) in item.iter_mut() {
            // A path item can carry `parameters` shared by every method on it, so
            // that key is walked too — not only the HTTP methods.
            if !is_http_method(method) && method != "parameters" {
                continue;
            }
            let params = if method == "parameters" {
                op.as_array_mut()
            } else {
                op.get_mut("parameters").and_then(Value::as_array_mut)
            };
            let Some(params) = params else { continue };

            for param in params.iter_mut() {
                let Some(target) = param
                    .get("$ref")
                    .and_then(Value::as_str)
                    .and_then(|r| r.strip_prefix("#/components/parameters/"))
                else {
                    continue;
                };
                match components.get(target) {
                    Some(body) => {
                        *param = body.clone();
                        stats.parameters_inlined += 1;
                    }
                    None => stats.unresolved += 1,
                }
            }
        }
    }

    stats
}

/// What [`resolve_response_refs`] rewired.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResponseRefStats {
    /// Component responses whose inline schema was given a name.
    pub components_hoisted: usize,
    /// Operation responses rewritten from a `$ref` into their real content.
    pub responses_inlined: usize,
}

/// Resolve `$ref`s into `components/responses` so operations carry their content
/// directly.
///
/// **WHY:** DigitalOcean writes every response as a reference —
/// `"202": { "$ref": "#/components/responses/droplet_create" }` — where Hetzner
/// inlines `content` at the operation. Our `Response` model has `description` and
/// `content` and **no `$ref` field**, so a DO response deserialises to an empty
/// `Response`, the endpoint's return type resolves to nothing, and every one of
/// its 447 paths generates as `ApiResponse<()>`. Silently: nothing errors, the
/// code compiles, and the client is useless.
///
/// This is the gap feature 00's verification list named ("DO's
/// `components/{responses,parameters,headers}` refs resolve — currently
/// unrepresentable in `Components`"). It is fixed here rather than in the model
/// because that is what canonicalisation is *for*: reshaping a vendor's document
/// into the one shape the generator understands, so the generator does not grow a
/// branch per vendor.
///
/// **WHAT:** two passes.
///
/// 1. Each `components/responses/<name>` with an **inline** schema gets that
///    schema hoisted to `components/schemas/<PascalName>` and left as a `$ref`.
///    Naming from the *component* is the point: a response shared by twenty
///    operations is one type with one meaningful name (`Unauthorized`), not
///    twenty copies named after whichever operation happened to hoist first.
/// 2. Each operation response that is a `$ref` into `components/responses` is
///    replaced by that component's body — which, after pass 1, carries only refs.
///
/// Run it **before** [`canonicalize_operations`], which then finds ordinary
/// `content` and has nothing left to do for these.
pub fn resolve_response_refs(spec: &mut Value) -> ResponseRefStats {
    let mut stats = ResponseRefStats::default();

    // ── pass 1: name the inline schemas inside components/responses ──────────
    let reserved: BTreeSet<String> = spec
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .map(|s| s.keys().map(|k| crate::api_catalog::to_pascal_case_from_any(k)).collect())
        .unwrap_or_default();

    let mut hoisted: Map<String, Value> = Map::new();
    if let Some(responses) = spec
        .pointer_mut("/components/responses")
        .and_then(Value::as_object_mut)
    {
        for (name, response) in responses.iter_mut() {
            let Some(schema) = response
                .get_mut("content")
                .and_then(Value::as_object_mut)
                .and_then(first_media_schema_mut)
            else {
                continue;
            };
            if !is_inline_object(schema) {
                continue; // already a $ref, or free-form
            }
            // `<Name>Response`, not `<Name>`. A vendor routinely keys a *request*
            // schema and a *response* the same: DigitalOcean has both
            // `components/schemas/droplet_create` (the body you send) and
            // `components/responses/droplet_create` (what comes back). Bare
            // pascal-casing collides, and the reserved-name guard then renames one
            // to `DropletCreate2` — correct, but it tells the reader nothing. The
            // suffix says what the type IS, and matches what
            // `canonicalize_operations` names an operation's response.
            let type_name = format!(
                "{}Response",
                crate::api_catalog::to_pascal_case_from_any(name)
            );
            let before = hoisted.len();
            let mut sink = CanonicalizeStats::default();
            hoist(schema, &type_name, &mut hoisted, &reserved, &mut sink);
            if hoisted.len() > before {
                stats.components_hoisted += 1;
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

    // ── pass 2: give each operation its response's body ──────────────────────
    let components: Map<String, Value> = spec
        .pointer("/components/responses")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if components.is_empty() {
        return stats;
    }

    let Some(paths) = spec.get_mut("paths").and_then(Value::as_object_mut) else {
        return stats;
    };

    for (_, item) in paths.iter_mut() {
        let Some(item) = item.as_object_mut() else { continue };
        for (method, op) in item.iter_mut() {
            if !is_http_method(method) {
                continue;
            }
            let Some(responses) = op
                .get_mut("responses")
                .and_then(Value::as_object_mut)
            else {
                continue;
            };

            for (_, response) in responses.iter_mut() {
                let Some(target) = response
                    .get("$ref")
                    .and_then(Value::as_str)
                    .and_then(|r| r.strip_prefix("#/components/responses/"))
                else {
                    continue;
                };
                // A `$ref` we cannot resolve is the vendor's bug, and not ours to
                // paper over — leave it exactly as it is so `dangling_refs`
                // reports it rather than us inventing an empty response.
                if let Some(body) = components.get(target) {
                    *response = body.clone();
                    stats.responses_inlined += 1;
                }
            }
        }
    }

    stats
}
