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

// ── canonicalize_operations ──────────────────────────────────────────────────

use foundation_openapi::canonicalize_operations;

/// A bundled spec: schemas written in place, the Linode/Hetzner shape.
fn bundled() -> serde_json::Value {
    json!({
        "openapi": "3.0.1",
        "servers": [{"url": "https://api.example.com"}],
        "paths": { "/instances": { "post": {
            "operationId": "post-linode-instance",
            "requestBody": { "content": { "application/json": {
                "schema": { "type": "object", "properties": { "label": { "type": "string" } } } } } },
            "responses": { "200": { "content": { "application/json": {
                "schema": { "type": "object", "properties": { "id": { "type": "integer" } } } } } } }
        }}},
        "components": { "schemas": {} }
    })
}

#[test]
fn hoists_inline_operation_schemas_and_leaves_refs() {
    let mut spec = bundled();
    assert!(validate_canonical(&spec).is_err(), "precondition: it starts inline");

    let stats = canonicalize_operations(&mut spec);

    assert_eq!(stats.hoisted, 2, "request + response");
    assert_eq!(validate_canonical(&spec), Ok(()), "now canonical");

    // The operation points at a ref…
    assert_eq!(
        spec["paths"]["/instances"]["post"]["responses"]["200"]["content"]["application/json"]
            ["schema"]["$ref"],
        "#/components/schemas/PostLinodeInstanceResponse"
    );
    // …and the schema it names actually exists, with its fields intact.
    assert_eq!(
        spec["components"]["schemas"]["PostLinodeInstanceResponse"]["properties"]["id"]["type"],
        "integer"
    );
}

#[test]
fn names_come_from_the_operation_id() {
    let mut spec = bundled();
    canonicalize_operations(&mut spec);
    let schemas = spec["components"]["schemas"].as_object().unwrap();

    assert!(schemas.contains_key("PostLinodeInstanceRequest"));
    assert!(schemas.contains_key("PostLinodeInstanceResponse"));
}

#[test]
fn without_an_operation_id_the_collection_and_item_paths_do_not_collide() {
    // `path_to_type_name` strips parameters, so /instances and /instances/{id}
    // both reduce to "Instances" — the method is what keeps them apart.
    let mut spec = json!({
        "openapi": "3.0.1",
        "servers": [{"url": "https://x"}],
        "paths": {
            "/instances": { "get": { "responses": { "200": { "content": { "application/json": {
                "schema": { "type": "object", "properties": { "a": { "type": "string" } } } } } } } } },
            "/instances/{id}": { "delete": { "responses": { "200": { "content": { "application/json": {
                "schema": { "type": "object", "properties": { "b": { "type": "string" } } } } } } } } }
        },
        "components": { "schemas": {} }
    });

    let stats = canonicalize_operations(&mut spec);
    assert_eq!(stats.hoisted, 2);
    assert_eq!(stats.renamed, 0, "different methods, different names — no dedup needed");

    let schemas = spec["components"]["schemas"].as_object().unwrap();
    assert!(schemas.contains_key("InstancesGetResponse"), "{:?}", schemas.keys().collect::<Vec<_>>());
    assert!(schemas.contains_key("InstancesDeleteResponse"));
}

#[test]
fn an_identical_shape_reuses_the_name_rather_than_duplicating() {
    // Same operationId stem, byte-identical schema: one type, not two.
    let mut spec = json!({
        "openapi": "3.0.1", "servers": [{"url": "https://x"}],
        "paths": { "/a": { "get": {
            "operationId": "thing",
            "responses": {
                "200": { "content": { "application/json": {
                    "schema": { "type": "object", "properties": { "x": { "type": "string" } } } } } },
                "201": { "content": { "application/json": {
                    "schema": { "type": "object", "properties": { "x": { "type": "string" } } } } } }
            }
        }}},
        "components": { "schemas": {} }
    });

    let stats = canonicalize_operations(&mut spec);
    assert_eq!(stats.renamed, 0, "identical shapes share a name");
    assert_eq!(validate_canonical(&spec), Ok(()));
}

#[test]
fn an_array_of_inline_objects_names_the_item() {
    let mut spec = json!({
        "openapi": "3.0.1", "servers": [{"url": "https://x"}],
        "paths": { "/list": { "get": {
            "operationId": "list-things",
            "responses": { "200": { "content": { "application/json": { "schema": {
                "type": "array",
                "items": { "type": "object", "properties": { "id": { "type": "integer" } } }
            }}}}}
        }}},
        "components": { "schemas": {} }
    });

    canonicalize_operations(&mut spec);

    assert_eq!(validate_canonical(&spec), Ok(()), "an array of $refs is canonical");
    assert_eq!(
        spec["paths"]["/list"]["get"]["responses"]["200"]["content"]["application/json"]["schema"]
            ["items"]["$ref"],
        "#/components/schemas/ListThingsResponseItem"
    );
}

#[test]
fn a_free_form_object_is_left_alone() {
    let mut spec = json!({
        "openapi": "3.0.1", "servers": [{"url": "https://x"}],
        "paths": { "/a": { "get": { "operationId": "a", "responses": { "200": {
            "content": { "application/json": { "schema": { "type": "object" } } } } } } } },
        "components": { "schemas": { "Keep": { "type": "object" } } }
    });

    let stats = canonicalize_operations(&mut spec);
    assert_eq!(stats.hoisted, 0, "there are no fields to name a type from");
}

/// The real thing: Linode's published spec is bundled, and this is what makes it
/// generatable at all.
#[test]
fn canonicalizes_the_real_linode_spec() {
    const SPEC: &str = "/home/darkvoid/Boxxed/@formulas/src.rust/src.cloud_providers/\
                        src.linode/linode-api-openapi/openapi.json";
    let Ok(raw) = std::fs::read_to_string(SPEC) else {
        eprintln!("SKIP: Linode spec not present at {SPEC}");
        return;
    };
    let mut spec: serde_json::Value = serde_json::from_str(&raw).expect("parses");

    ensure_servers(&mut spec, "https://api.linode.com");
    let stats = canonicalize_operations(&mut spec);

    assert!(stats.hoisted > 500, "the bundled spec is full of them: {}", stats.hoisted);
    assert_eq!(
        validate_canonical(&spec),
        Ok(()),
        "after canonicalising, Linode's spec is in the shape the generator needs"
    );
}

/// The pipeline as feature 00 actually runs it: **select, then canonicalise**.
///
/// Canonicalising the whole 334-path spec hoists ~1042 schemas; selecting the six
/// endpoints we use first means we carry a fraction of that. This is the payoff.
#[test]
fn select_then_canonicalize_keeps_the_surface_small() {
    const SPEC: &str = "/home/darkvoid/Boxxed/@formulas/src.rust/src.cloud_providers/\
                        src.linode/linode-api-openapi/openapi.json";
    let Ok(raw) = std::fs::read_to_string(SPEC) else {
        eprintln!("SKIP: Linode spec not present");
        return;
    };
    let mut spec: serde_json::Value = serde_json::from_str(&raw).expect("parses");

    // Selection, applied to the raw JSON: keep only the paths feature 03 needs.
    let wanted = [
        "/{apiVersion}/linode/instances",
        "/{apiVersion}/linode/instances/{linodeId}",
        "/{apiVersion}/profile/sshkeys",
    ];
    let paths = spec["paths"].as_object_mut().expect("paths");
    let before = paths.len();
    paths.retain(|p, _| wanted.contains(&p.as_str()));
    assert_eq!(paths.len(), 3, "selected 3 of {before}");

    ensure_servers(&mut spec, "https://api.linode.com");
    let stats = canonicalize_operations(&mut spec);

    assert_eq!(validate_canonical(&spec), Ok(()), "canonical");
    assert!(
        stats.hoisted < 40,
        "3 paths should hoist a handful of schemas, not the whole spec's 1042 — got {}",
        stats.hoisted
    );
}

// ── strip_doc_only ───────────────────────────────────────────────────────────

#[test]
fn strips_documentation_that_generates_nothing_and_keeps_what_does() {
    use foundation_openapi::strip_doc_only;

    let mut spec = json!({
        "paths": { "/a": { "get": {
            "summary": "keep me",
            "description": "keep me too — I become a doc comment",
            "x-codeSamples": [{ "lang": "curl", "source": "curl -d url=https://hooks.slack.com/services/T00/B00/XXX" }],
            "responses": { "200": { "content": { "application/json": {
                "schema": { "type": "object",
                            "example": { "url": "https://hooks.slack.com/services/T00/B00/XXX" },
                            "properties": { "url": { "type": "string", "example": "https://hooks.slack.com/services/T00/B00/XXX" } } } } } } }
        }}},
        "components": { "examples": { "Sample": { "value": "gone" } } }
    });

    strip_doc_only(&mut spec);

    let out = serde_json::to_string(&spec).unwrap();
    // The reason this exists: vendor doc placeholders trip secret scanning on an
    // artefact we have to commit.
    assert!(!out.contains("hooks.slack.com"), "every doc placeholder is gone");
    assert!(!out.contains("x-codeSamples"));
    assert!(!out.contains("\"example\""));

    // …but the parts that generate code survive.
    assert_eq!(spec["paths"]["/a"]["get"]["summary"], "keep me");
    assert!(spec["paths"]["/a"]["get"]["description"].is_string(), "becomes a doc comment");
    assert_eq!(
        spec["paths"]["/a"]["get"]["responses"]["200"]["content"]["application/json"]["schema"]
            ["properties"]["url"]["type"],
        "string",
        "the property itself stays — only its example went"
    );
}

#[test]
fn strip_doc_only_is_idempotent() {
    use foundation_openapi::strip_doc_only;
    // It runs at vendoring time and again in `normalize`; the second must be a
    // no-op or the artefact would not be reproducible.
    let mut spec = json!({ "paths": { "/a": { "get": { "example": "x", "summary": "s" } } } });
    strip_doc_only(&mut spec);
    let once = spec.clone();
    strip_doc_only(&mut spec);
    assert_eq!(spec, once);
}

// ── component names must be resolvable pointers ──────────────────────────────

#[test]
fn a_property_key_with_a_slash_still_yields_a_resolvable_ref() {
    // Cloudflare's real bug: a property keyed `application/json` produced the
    // component `…ContentApplication/json`, whose $ref cannot resolve — the slash
    // reads as a JSON-pointer separator. The schema is written and every
    // reference to it dangles.
    let mut props = json!({
        "content": { "type": "object", "properties": {
            "application/json": { "type": "object", "properties": {
                "schema": { "type": "object", "properties": { "id": { "type": "string" } } } } } } }
    })
    .as_object()
    .unwrap()
    .clone();
    let mut schemas = serde_json::Map::new();

    extract_inline_schemas(&mut props, "Resp", &mut schemas);

    for name in schemas.keys() {
        assert!(
            !name.contains('/'),
            "a component name is a pointer segment: {name}"
        );
    }
    assert!(
        schemas.contains_key("RespContentApplicationJson"),
        "got {:?}",
        schemas.keys().collect::<Vec<_>>()
    );

    // The invariant that actually matters: every ref resolves.
    let spec = json!({ "paths": { "/a": { "get": { "responses": { "200": {
        "content": { "application/json": { "schema": { "type": "object", "properties": props } } } } } } } },
        "components": { "schemas": schemas } });
    assert_eq!(foundation_openapi::dangling_refs(&spec), Vec::<String>::new());
}

#[test]
fn a_property_key_starting_with_a_multibyte_char_does_not_panic() {
    // The old name builder uppercased byte 0 and sliced from byte 1 — which
    // panics outright on a non-ASCII key rather than producing a bad name.
    let mut props = json!({ "é_field": { "type": "object", "properties": { "x": { "type": "string" } } } })
        .as_object()
        .unwrap()
        .clone();
    let mut schemas = serde_json::Map::new();
    extract_inline_schemas(&mut props, "T", &mut schemas);
    assert_eq!(schemas.len(), 1, "it named something rather than panicking");
}

/// Cloudflare's real 19 MB spec — the document that exposed the slash bug.
///
/// Skips if the artefact is absent.
#[test]
fn canonicalising_the_real_cloudflare_spec_leaves_every_hoisted_ref_resolvable() {
    const SPEC: &str = "../../artefacts/cloud_providers/cloudflare/openapi.json";
    if !std::path::Path::new(SPEC).exists() {
        eprintln!("SKIP: cloudflare artefact not present");
        return;
    }
    let mut spec: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(SPEC).unwrap()).unwrap();

    let before = foundation_openapi::dangling_refs(&spec);
    let stats = foundation_openapi::canonicalize_operations(&mut spec);
    let after = foundation_openapi::dangling_refs(&spec);

    // Canonicalising must not *introduce* a dangling ref. Cloudflare ships one of
    // its own (`abuse-reports_CSAMReport`, referenced once and declared nowhere),
    // so the assertion is "no new ones", not "none".
    let introduced: Vec<&String> = after.iter().filter(|r| !before.contains(r)).collect();
    assert!(
        introduced.is_empty(),
        "hoisting {} schemas introduced {} unresolvable refs: {:?}",
        stats.hoisted,
        introduced.len(),
        &introduced[..introduced.len().min(5)]
    );
    eprintln!(
        "  cloudflare: hoisted {} (renamed {}), vendor's own dangling refs: {}",
        stats.hoisted,
        stats.renamed,
        before.len()
    );
}

#[test]
fn hoisting_never_takes_a_name_the_vendor_already_owns() {
    // Cloudflare's real collision: the spec bundles `vectorize_index_info_response`
    // AND has an operation `vectorize-index-info`, whose response we would name
    // `VectorizeIndexInfoResponse` — the same Rust type after pascal-casing. The
    // field referencing the vendor's schema then renders as its own parent, which
    // the compiler reports as "recursive type has infinite size". It is really two
    // different schemas fighting over one name, and silently picking a winner
    // would be worse than the error.
    let mut spec = json!({
        "paths": { "/info": { "get": {
            "operationId": "vectorize-index-info",
            "responses": { "200": { "content": { "application/json": {
                "schema": { "type": "object", "properties": {
                    "result": { "$ref": "#/components/schemas/vectorize_index_info_response" } } } } } } }
        }}},
        "components": { "schemas": {
            "vectorize_index_info_response": { "type": "object", "properties": { "dimensions": { "type": "integer" } } }
        }}
    });

    let stats = foundation_openapi::canonicalize_operations(&mut spec);

    // The hoisted response had to take a different name.
    let schemas = spec["components"]["schemas"].as_object().unwrap();
    assert!(
        schemas.contains_key("VectorizeIndexInfoResponse2"),
        "the hoisted response stepped aside; got {:?}",
        schemas.keys().collect::<Vec<_>>()
    );
    assert_eq!(stats.renamed, 1, "and it counted as a rename");

    // The vendor's schema is untouched, and the operation points at ours.
    assert!(schemas.contains_key("vectorize_index_info_response"));
    assert_eq!(
        spec["paths"]["/info"]["get"]["responses"]["200"]["content"]["application/json"]["schema"]
            ["$ref"],
        "#/components/schemas/VectorizeIndexInfoResponse2"
    );
    assert_eq!(foundation_openapi::dangling_refs(&spec), Vec::<String>::new());
}

#[test]
fn an_identical_shape_hoisted_twice_shares_one_name() {
    // The flip side: renaming must not be indiscriminate. The same shape under two
    // operations is one type, and two names for it would emit the struct twice.
    let mut spec = json!({
        "paths": {
            "/a": { "get": { "operationId": "op-a", "responses": { "200": { "content": { "application/json": {
                "schema": { "type": "object", "properties": { "id": { "type": "string" } } } } } } } } },
            "/b": { "get": { "operationId": "op-a", "responses": { "200": { "content": { "application/json": {
                "schema": { "type": "object", "properties": { "id": { "type": "string" } } } } } } } } }
        },
        "components": { "schemas": {} }
    });

    let stats = foundation_openapi::canonicalize_operations(&mut spec);
    assert_eq!(stats.renamed, 0, "the same shape twice is one type");
    assert_eq!(
        spec["components"]["schemas"].as_object().unwrap().len(),
        1,
        "one struct, not two"
    );
}

// ── responses behind a $ref ──────────────────────────────────────────────────

#[test]
fn a_response_referenced_from_components_gets_its_content_inlined() {
    use foundation_openapi::resolve_response_refs;

    // DigitalOcean's shape: every response is `{"$ref": "#/components/responses/X"}`.
    // Our `Response` model has `description` and `content` and NO `$ref` field, so
    // this deserialises to an empty Response, the return type resolves to nothing,
    // and all 447 of DO's paths generate as `ApiResponse<()>` — silently, with the
    // body discarded.
    let mut spec = json!({
        "paths": { "/droplets": { "post": {
            "operationId": "droplets_create",
            "responses": { "202": { "$ref": "#/components/responses/droplet_create" } }
        }}},
        "components": {
            "responses": { "droplet_create": { "content": { "application/json": {
                "schema": { "type": "object", "properties": { "droplet": { "type": "string" } } } } } } },
            "schemas": {}
        }
    });

    let stats = resolve_response_refs(&mut spec);
    assert_eq!(stats.components_hoisted, 1);
    assert_eq!(stats.responses_inlined, 1);

    // The operation now carries content the generator can see.
    let schema = &spec["paths"]["/droplets"]["post"]["responses"]["202"]["content"]
        ["application/json"]["schema"];
    assert_eq!(schema["$ref"], "#/components/schemas/DropletCreateResponse");
    assert!(spec["components"]["schemas"]["DropletCreateResponse"].is_object());
    assert_eq!(foundation_openapi::dangling_refs(&spec), Vec::<String>::new());
}

#[test]
fn a_response_component_is_named_once_not_once_per_operation() {
    use foundation_openapi::resolve_response_refs;

    // The reason names come from the COMPONENT and not the operation: an error
    // response shared by twenty endpoints should be one type with one meaningful
    // name, not twenty copies named after whichever operation hoisted first.
    let mut spec = json!({
        "paths": {
            "/a": { "get": { "operationId": "a", "responses": { "401": { "$ref": "#/components/responses/unauthorized" } } } },
            "/b": { "get": { "operationId": "b", "responses": { "401": { "$ref": "#/components/responses/unauthorized" } } } }
        },
        "components": {
            "responses": { "unauthorized": { "content": { "application/json": {
                "schema": { "type": "object", "properties": { "id": { "type": "string" } } } } } } },
            "schemas": {}
        }
    });

    resolve_response_refs(&mut spec);

    let schemas = spec["components"]["schemas"].as_object().unwrap();
    assert_eq!(schemas.len(), 1, "one type, not one per operation: {:?}", schemas.keys().collect::<Vec<_>>());
    assert!(schemas.contains_key("UnauthorizedResponse"));
    for path in ["/a", "/b"] {
        assert_eq!(
            spec["paths"][path]["get"]["responses"]["401"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/UnauthorizedResponse"
        );
    }
}

#[test]
fn a_response_component_never_steals_a_schemas_name() {
    use foundation_openapi::resolve_response_refs;

    // DigitalOcean has BOTH `components/schemas/droplet_create` (the request body)
    // and `components/responses/droplet_create` (what comes back). Bare
    // pascal-casing collides; the `Response` suffix says which is which.
    let mut spec = json!({
        "paths": { "/d": { "post": {
            "operationId": "droplets_create",
            "responses": { "202": { "$ref": "#/components/responses/droplet_create" } }
        }}},
        "components": {
            "responses": { "droplet_create": { "content": { "application/json": {
                "schema": { "type": "object", "properties": { "droplet": { "type": "string" } } } } } } },
            "schemas": { "droplet_create": { "type": "object", "properties": { "name": { "type": "string" } } } }
        }
    });

    resolve_response_refs(&mut spec);

    let schemas = spec["components"]["schemas"].as_object().unwrap();
    assert!(schemas.contains_key("droplet_create"), "the vendor's request schema is untouched");
    assert!(schemas.contains_key("DropletCreateResponse"), "and the response has its own name");
    assert!(
        !schemas.keys().any(|k| k.ends_with('2')),
        "no fallback rename was needed: {:?}",
        schemas.keys().collect::<Vec<_>>()
    );
}

#[test]
fn a_oneof_response_is_named_rather_than_dropped() {
    use foundation_openapi::resolve_response_refs;

    // DO's droplet_create response is a `oneOf` (one droplet, or many). We do not
    // generate Rust enums for unions — the type comes out as a flattened
    // HashMap — but hoisting it anyway is the difference between handing the
    // caller the JSON and handing them `()` with the body discarded.
    let mut spec = json!({
        "paths": { "/d": { "post": {
            "operationId": "c",
            "responses": { "202": { "$ref": "#/components/responses/droplet_create" } }
        }}},
        "components": {
            "responses": { "droplet_create": { "content": { "application/json": { "schema": { "oneOf": [
                { "type": "object", "properties": { "droplet": { "type": "string" } } },
                { "type": "object", "properties": { "droplets": { "type": "array", "items": { "type": "string" } } } }
            ]}}}}},
            "schemas": {}
        }
    });

    resolve_response_refs(&mut spec);
    assert!(
        spec["components"]["schemas"]["DropletCreateResponse"].is_object(),
        "the union got a name"
    );
}

#[test]
fn a_response_ref_the_vendor_never_defined_is_left_for_the_dangling_check() {
    use foundation_openapi::resolve_response_refs;

    // Not ours to paper over: inventing an empty response here would hide the
    // vendor's bug and generate a client that silently returns nothing.
    let mut spec = json!({
        "paths": { "/a": { "get": { "operationId": "a", "responses": {
            "200": { "$ref": "#/components/responses/never_defined" } } } } },
        "components": { "responses": {}, "schemas": {} }
    });

    let stats = resolve_response_refs(&mut spec);
    assert_eq!(stats.responses_inlined, 0);
    assert_eq!(
        spec["paths"]["/a"]["get"]["responses"]["200"]["$ref"],
        "#/components/responses/never_defined",
        "left exactly as the vendor wrote it"
    );
}

/// DigitalOcean's real 447-path spec — the document that exposed this.
#[test]
fn the_real_digitalocean_responses_resolve() {
    use foundation_openapi::resolve_response_refs;

    const SPEC: &str = "../../artefacts/cloud_providers/digitalocean/openapi.json";
    if !std::path::Path::new(SPEC).exists() {
        eprintln!("SKIP: digitalocean artefact not present");
        return;
    }
    let mut spec: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(SPEC).unwrap()).unwrap();

    let before = foundation_openapi::dangling_refs(&spec);
    let stats = resolve_response_refs(&mut spec);
    let after = foundation_openapi::dangling_refs(&spec);

    assert!(stats.responses_inlined > 100, "DO refs its responses everywhere: {stats:?}");
    let introduced: Vec<&String> = after.iter().filter(|r| !before.contains(r)).collect();
    assert!(introduced.is_empty(), "rewiring must not break a ref: {introduced:?}");
    eprintln!(
        "  digitalocean: {} component responses named, {} operation responses rewired",
        stats.components_hoisted, stats.responses_inlined
    );
}

// ── parameters behind a $ref ─────────────────────────────────────────────────

#[test]
fn a_parameter_referenced_from_components_is_inlined() {
    use foundation_openapi::resolve_parameter_refs;

    // DigitalOcean writes EVERY parameter as a reference, and our `Parameter`
    // model has name/in/required/schema and NO $ref field — so each one
    // deserialised into a nameless parameter and was dropped. 833 of DO's 1030
    // vanished, silently, and `droplets_list` generated with no arguments at all.
    //
    // Not cosmetic: `?tag_name=` is how a deployment enumerates its OWN droplets
    // (DigitalOcean's answer to Hetzner's label selector), so losing it took the
    // identity mechanism with it.
    let mut spec = json!({
        "paths": { "/v2/droplets": { "get": {
            "operationId": "droplets_list",
            "parameters": [
                { "$ref": "#/components/parameters/droplet_tag_name" },
                { "name": "inline_one", "in": "query", "schema": { "type": "string" } }
            ],
            "responses": {}
        }}},
        "components": { "parameters": { "droplet_tag_name": {
            "name": "tag_name", "in": "query", "schema": { "type": "string" }
        }}}
    });

    let stats = resolve_parameter_refs(&mut spec);
    assert_eq!(stats.parameters_inlined, 1);
    assert_eq!(stats.unresolved, 0);

    let params = spec["paths"]["/v2/droplets"]["get"]["parameters"].as_array().unwrap();
    assert_eq!(params[0]["name"], "tag_name", "the ref became the real parameter");
    assert_eq!(params[0]["in"], "query");
    assert_eq!(params[1]["name"], "inline_one", "an inline parameter is untouched");
}

#[test]
fn parameters_shared_by_a_whole_path_item_are_resolved_too() {
    use foundation_openapi::resolve_parameter_refs;

    // A path item may carry `parameters` that apply to every method on it. Walking
    // only the HTTP methods would leave those refs dangling.
    let mut spec = json!({
        "paths": { "/v2/droplets/{id}": {
            "parameters": [{ "$ref": "#/components/parameters/droplet_id" }],
            "get": { "operationId": "droplets_get", "responses": {} }
        }},
        "components": { "parameters": { "droplet_id": {
            "name": "droplet_id", "in": "path", "required": true, "schema": { "type": "integer" }
        }}}
    });

    let stats = resolve_parameter_refs(&mut spec);
    assert_eq!(stats.parameters_inlined, 1);
    assert_eq!(
        spec["paths"]["/v2/droplets/{id}"]["parameters"][0]["name"],
        "droplet_id"
    );
}

#[test]
fn a_parameter_ref_the_vendor_never_defined_is_left_alone() {
    use foundation_openapi::resolve_parameter_refs;

    // The vendor's bug. Inventing an empty parameter would hide it and generate a
    // call that silently omits an argument.
    let mut spec = json!({
        "paths": { "/a": { "get": {
            "operationId": "a",
            "parameters": [{ "$ref": "#/components/parameters/never_defined" }],
            "responses": {}
        }}},
        "components": { "parameters": { "other": { "name": "x", "in": "query" } } }
    });

    let stats = resolve_parameter_refs(&mut spec);
    assert_eq!(stats.parameters_inlined, 0);
    assert_eq!(stats.unresolved, 1);
    assert_eq!(
        spec["paths"]["/a"]["get"]["parameters"][0]["$ref"],
        "#/components/parameters/never_defined",
        "left exactly as the vendor wrote it"
    );
}

/// DigitalOcean's real spec — the document that exposed this.
#[test]
fn the_real_digitalocean_parameters_resolve() {
    use foundation_openapi::resolve_parameter_refs;

    const SPEC: &str = "../../artefacts/cloud_providers/digitalocean/openapi.json";
    if !std::path::Path::new(SPEC).exists() {
        eprintln!("SKIP: digitalocean artefact not present");
        return;
    }
    let mut spec: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(SPEC).unwrap()).unwrap();

    let before = foundation_openapi::dangling_refs(&spec);
    let stats = resolve_parameter_refs(&mut spec);
    let after = foundation_openapi::dangling_refs(&spec);

    assert!(stats.parameters_inlined > 800, "DO refs nearly all of them: {stats:?}");
    let introduced: Vec<&String> = after.iter().filter(|r| !before.contains(r)).collect();
    assert!(introduced.is_empty(), "rewiring must not break a ref: {introduced:?}");

    // The one that carries the identity mechanism.
    let list = &spec["paths"]["/v2/droplets"]["get"]["parameters"];
    let names: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p["name"].as_str())
        .collect();
    assert!(
        names.contains(&"tag_name"),
        "droplets_list must keep ?tag_name= — it is how a deployment finds its own droplets: {names:?}"
    );
    eprintln!("  digitalocean: {} parameters rewired", stats.parameters_inlined);
}
