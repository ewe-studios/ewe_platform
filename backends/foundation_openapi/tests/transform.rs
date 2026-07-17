use foundation_openapi::transform::{
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

// ── validate_canonical (spec-56 decision 05) ─────────────────────────────────
//
// WHY these exist: a half-applied transform yields a spec that generates quietly
// wrong code. `normalize` running some transforms and hoping is exactly how a
// crate ends up "code present" and broken — the pattern spec-53's audit kept
// finding. Validation is the cheap proof the transforms did their job.

use foundation_openapi::transform::{validate_canonical, NotCanonical};

/// A minimal spec that is genuinely canonical.
fn canonical() -> serde_json::Value {
    json!({
        "openapi": "3.0.3",
        "servers": [{"url": "https://api.example.com"}],
        "paths": {
            "/servers": {
                "post": {
                    "requestBody": { "content": { "application/json": {
                        "schema": { "$ref": "#/components/schemas/CreateServer" } } } },
                    "responses": { "201": { "content": { "application/json": {
                        "schema": { "$ref": "#/components/schemas/Server" } } } } }
                }
            }
        },
        "components": { "schemas": {
            "Server": { "type": "object", "properties": { "id": { "type": "integer" } } },
            "CreateServer": { "type": "object", "properties": { "name": { "type": "string" } } }
        }}
    })
}

#[test]
fn a_canonical_spec_passes() {
    assert_eq!(validate_canonical(&canonical()), Ok(()));
}

#[test]
fn missing_servers_is_caught() {
    let mut spec = canonical();
    spec.as_object_mut().unwrap().remove("servers");
    let issues = validate_canonical(&spec).expect_err("should not be canonical");
    assert!(issues.contains(&NotCanonical::NoServers));
}

#[test]
fn empty_servers_is_caught() {
    let mut spec = canonical();
    spec["servers"] = json!([]);
    assert!(validate_canonical(&spec)
        .expect_err("should not be canonical")
        .contains(&NotCanonical::NoServers));
}

#[test]
fn missing_component_schemas_is_caught() {
    let mut spec = canonical();
    spec["components"] = json!({});
    assert!(validate_canonical(&spec)
        .expect_err("should not be canonical")
        .contains(&NotCanonical::NoComponentSchemas));
}

#[test]
fn an_inline_response_schema_is_caught() {
    // This is the Linode/Hetzner shape: a bundled spec, schemas written in place.
    // The extractor names types from `$ref`, so this generates as an anonymous
    // blob — the whole reason the transform stage exists.
    let mut spec = canonical();
    spec["paths"]["/servers"]["post"]["responses"]["201"]["content"]["application/json"]
        ["schema"] = json!({ "type": "object", "properties": { "id": { "type": "integer" } } });

    let issues = validate_canonical(&spec).expect_err("inline schema is not canonical");
    assert!(issues.iter().any(|i| matches!(
        i,
        NotCanonical::InlineSchema { path, method, location }
            if path == "/servers" && method == "POST" && location == "201"
    )), "should name where the inline schema is: {issues:?}");
}

#[test]
fn an_inline_request_body_is_caught() {
    let mut spec = canonical();
    spec["paths"]["/servers"]["post"]["requestBody"]["content"]["application/json"]["schema"] =
        json!({ "type": "object", "properties": { "name": { "type": "string" } } });

    let issues = validate_canonical(&spec).expect_err("inline requestBody is not canonical");
    assert!(issues.iter().any(|i| matches!(
        i,
        NotCanonical::InlineSchema { location, .. } if location == "requestBody"
    )), "{issues:?}");
}

#[test]
fn an_inline_allof_is_caught() {
    // Linode's real shape: `POST /{apiVersion}/linode/instances` has a requestBody
    // schema of `{"allOf": [...], "type": "object"}` written in place.
    let mut spec = canonical();
    spec["paths"]["/servers"]["post"]["requestBody"]["content"]["application/json"]["schema"] =
        json!({ "type": "object", "allOf": [{ "properties": { "a": { "type": "string" } } }] });

    assert!(validate_canonical(&spec).is_err(), "allOf composed in place is inline too");
}

#[test]
fn an_array_of_inline_objects_is_caught() {
    let mut spec = canonical();
    spec["paths"]["/servers"]["post"]["responses"]["201"]["content"]["application/json"]
        ["schema"] = json!({
            "type": "array",
            "items": { "type": "object", "properties": { "id": { "type": "integer" } } }
        });

    assert!(validate_canonical(&spec).is_err(), "inline items are inline");
}

#[test]
fn an_array_of_refs_is_fine() {
    let mut spec = canonical();
    spec["paths"]["/servers"]["post"]["responses"]["201"]["content"]["application/json"]
        ["schema"] = json!({
            "type": "array", "items": { "$ref": "#/components/schemas/Server" }
        });

    assert_eq!(validate_canonical(&spec), Ok(()), "a list of $refs is canonical");
}

#[test]
fn a_free_form_object_is_fine() {
    // `{"type": "object"}` with no properties is a free-form map — there are no
    // fields to name a type from, so it is not the problem this catches.
    let mut spec = canonical();
    spec["paths"]["/servers"]["post"]["responses"]["201"]["content"]["application/json"]
        ["schema"] = json!({ "type": "object" });

    assert_eq!(validate_canonical(&spec), Ok(()));
}

#[test]
fn every_problem_is_reported_not_just_the_first() {
    // One pass should tell you everything to fix.
    let mut spec = canonical();
    spec.as_object_mut().unwrap().remove("servers");
    spec["components"] = json!({});
    spec["paths"]["/servers"]["post"]["responses"]["201"]["content"]["application/json"]
        ["schema"] = json!({ "type": "object", "properties": { "id": { "type": "integer" } } });

    let issues = validate_canonical(&spec).expect_err("three problems");
    assert!(issues.len() >= 3, "expected all three, got: {issues:?}");
}

/// The premise of the whole transform stage, pinned against a real spec.
///
/// Linode's published spec is fully bundled — 0 `$ref`s, every schema written in
/// place. Our extractor names a response's type from its `ref_path`, so without
/// hoisting, its endpoints generate as anonymous blobs. If this ever starts
/// passing, Linode changed their spec style and the Linode normalizer can go.
///
/// Skips when the owner-supplied clone is absent.
#[test]
fn the_real_linode_spec_is_not_canonical_for_us() {
    const SPEC: &str = "/home/darkvoid/Boxxed/@formulas/src.rust/src.cloud_providers/\
                        src.linode/linode-api-openapi/openapi.json";
    let Ok(raw) = std::fs::read_to_string(SPEC) else {
        eprintln!("SKIP: Linode spec not present at {SPEC}");
        return;
    };
    let spec: serde_json::Value = serde_json::from_str(&raw).expect("linode spec parses");

    let issues = validate_canonical(&spec)
        .expect_err("Linode's spec is bundled — it should not pass as canonical");

    let inline = issues
        .iter()
        .filter(|i| matches!(i, NotCanonical::InlineSchema { .. }))
        .count();
    assert!(
        inline > 500,
        "expected the bundled spec to be full of inline schemas, found {inline}"
    );
}
