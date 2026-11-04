//! Overlays, typed `oneOf` unions and schemas hoisted into the shared module
//! (feature 52).
//!
//! WHY: Cloudflare's D1 `/query` takes a `oneOf` body (one query, or a batch)
//! that generated as `serde_json::Value`, so `foundation_db` built request
//! bodies by hand and hand-wrote the D1 and R2 response types. Each rule below
//! is what lets those types come from the generator instead.
//!
//! WHAT: A small D1-shaped spec goes through [`Pipeline`] and
//! [`UnifiedGenerator`]; the tests read the emitted source.

use foundation_openapi::overlay::{apply, merge_patch};
use foundation_openapi::unified::{AnalysisOptions, UnifiedGenerator};
use foundation_openapi::{OverlayError, Pipeline};
use serde_json::json;
use std::path::{Path, PathBuf};

fn tmpdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ewe-overlay-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// One endpoint taking a `oneOf` body (a single query, or a batch) and
/// returning an object.
fn query_spec() -> serde_json::Value {
    json!({
        "openapi": "3.0.0",
        "info": {"title": "q", "version": "1"},
        "servers": [{"url": "http://localhost"}],
        "paths": {
            "/db/query": {"post": {
                "operationId": "db-query",
                "requestBody": {"required": true, "content": {"application/json": {
                    "schema": {"$ref": "#/components/schemas/batch_query"}
                }}},
                "responses": {"200": {"description": "ok", "content": {"application/json": {
                    "schema": {"$ref": "#/components/schemas/query_result"}
                }}}}
            }}
        },
        "components": {"schemas": {
            "batch_query": {
                "description": "A single query object or a batch query object",
                "oneOf": [
                    {"$ref": "#/components/schemas/single_query"},
                    {
                        "type": "object",
                        "title": "multiple queries",
                        "required": ["batch"],
                        "properties": {"batch": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/single_query"}
                        }}
                    }
                ]
            },
            "single_query": {
                "type": "object",
                "required": ["sql"],
                "properties": {"sql": {"type": "string"}, "params": {
                    "type": "array", "items": {"type": "string"}
                }}
            },
            "query_result": {
                "type": "object",
                "properties": {"rows": {"type": "integer"}}
            }
        }}
    })
}

/// Generate `spec` and return `(relative path, source)` of every module.
fn generate(
    tag: &str,
    spec: &serde_json::Value,
    typed_unions: &[&str],
    options: &AnalysisOptions,
) -> Vec<(String, String)> {
    let out = std::env::temp_dir().join(format!(
        "ewe-overlay-gen-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let src = out.join("src");
    UnifiedGenerator::new(out.clone())
        .with_provider_dir(src.clone())
        .with_typed_unions(typed_unions.iter().copied())
        .generate(tag, &spec.to_string(), options)
        .expect("generation succeeds");
    let generated = src.join("generated");
    let mut modules = Vec::new();
    for entry in std::fs::read_dir(&generated).unwrap() {
        let module = entry.unwrap().path().join("mod.rs");
        if module.exists() {
            let name = module
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            modules.push((name, std::fs::read_to_string(&module).unwrap()));
        }
    }
    modules.sort();
    std::fs::remove_dir_all(&out).unwrap();
    modules
}

fn all_source(modules: &[(String, String)]) -> String {
    modules.iter().map(|(_, s)| s.as_str()).collect()
}

// ── overlays ─────────────────────────────────────────────────────────────────

#[test]
fn merge_patch_follows_rfc_7386() {
    let mut target = json!({"a": {"b": 1, "c": 2}, "list": [1, 2], "gone": true});
    merge_patch(
        &mut target,
        &json!({"a": {"b": null, "d": 4}, "list": [9], "gone": null}),
    );
    // Objects merge, `null` removes a key, an array replaces the whole value.
    assert_eq!(target, json!({"a": {"c": 2, "d": 4}, "list": [9]}));
}

#[test]
fn an_overlay_must_say_where_it_came_from() {
    let mut spec = json!({});
    for (overlay, missing) in [
        (json!({"source": "x", "patch": {}}), "description"),
        (json!({"description": "x", "patch": {}}), "source"),
        (json!({"description": "x", "source": " "}), "source"),
        (
            json!({"description": "x", "source": "y", "patch": 3}),
            "patch",
        ),
    ] {
        match apply(&mut spec, &overlay, Path::new("o.json")) {
            Err(OverlayError::Malformed { missing: m, .. }) => {
                assert!(m.contains(missing), "{m} should name {missing}");
            }
            other => panic!("expected Malformed for {overlay}, got {other:?}"),
        }
    }
    assert_eq!(spec, json!({}), "a rejected overlay changes nothing");
}

#[test]
fn the_pipeline_merges_overlays_in_order_before_selection() {
    let dir = tmpdir("pipeline");
    let spec_path = dir.join("spec.json");
    std::fs::write(&spec_path, query_spec().to_string()).unwrap();
    let first = dir.join("1.json");
    std::fs::write(
        &first,
        json!({
            "description": "adds a path", "source": "test",
            "patch": {"paths": {"/db/extra": {"get": {
                "operationId": "db-extra",
                "responses": {"200": {"description": "ok", "content": {"application/json": {
                    "schema": {"$ref": "#/components/schemas/query_result"}}}}}
            }}}}
        })
        .to_string(),
    )
    .unwrap();
    let second = dir.join("2.json");
    std::fs::write(
        &second,
        json!({
            "description": "retypes a field", "source": "test",
            "patch": {"components": {"schemas": {"query_result": {
                "properties": {"rows": {"type": "number"}}}}}}
        })
        .to_string(),
    )
    .unwrap();

    let resolved = Pipeline::new("test", &spec_path)
        .overlay(&first)
        .overlay(&second)
        .resolve()
        .expect("resolves");
    assert!(resolved.pointer("/paths/~1db~1extra/get").is_some());
    assert_eq!(
        resolved.pointer("/components/schemas/query_result/properties/rows/type"),
        Some(&json!("number"))
    );

    let missing = Pipeline::new("test", &spec_path)
        .overlay(dir.join("nope.json"))
        .resolve();
    assert!(
        matches!(missing, Err(foundation_openapi::PipelineError::Overlay(_))),
        "an unreadable overlay is an error, not a silent skip"
    );
}

// ── typed unions ─────────────────────────────────────────────────────────────

#[test]
fn a_named_one_of_becomes_an_untagged_enum() {
    let modules = generate(
        "unions_named",
        &query_spec(),
        &["batch_query"],
        &AnalysisOptions::default(),
    );
    let src = all_source(&modules);
    assert!(
        src.contains("#[serde(untagged)]\npub enum BatchQuery {"),
        "no enum:\n{src}"
    );
    assert!(src.contains("SingleQuery(SingleQuery),"), "{src}");
    assert!(
        src.contains("MultipleQueries {"),
        "an inline member is named after its title:\n{src}"
    );
    assert!(src.contains("batch: Vec<SingleQuery>,"), "{src}");
    assert!(
        src.contains("impl Default for BatchQuery"),
        "a union used as an args body needs Default:\n{src}"
    );
    assert!(
        src.contains("pub body: BatchQuery,"),
        "the endpoint takes the enum, not Value:\n{src}"
    );
}

#[test]
fn a_one_of_not_named_stays_a_json_value() {
    let modules = generate(
        "unions_unnamed",
        &query_spec(),
        &[],
        &AnalysisOptions::default(),
    );
    let src = all_source(&modules);
    assert!(!src.contains("pub enum BatchQuery"), "{src}");
    assert!(
        src.contains("pub type BatchQuery = serde_json::Value;"),
        "{src}"
    );
}

// ── hoisting into shared ─────────────────────────────────────────────────────

#[test]
fn a_hoisted_schema_and_what_it_references_land_in_shared() {
    let options = AnalysisOptions {
        hoist_to_shared: vec!["batch_query".to_string()],
        ..AnalysisOptions::default()
    };
    let modules = generate("hoist", &query_spec(), &["batch_query"], &options);
    let shared = &modules.iter().find(|(n, _)| n == "shared").unwrap().1;
    assert!(shared.contains("pub enum BatchQuery"), "{shared}");
    assert!(
        shared.contains("pub struct SingleQuery"),
        "a hoisted type brings the types it references:\n{shared}"
    );
    let group = modules
        .iter()
        .filter(|(n, _)| n != "shared")
        .map(|(_, s)| s.as_str())
        .collect::<String>();
    assert!(
        !group.contains("pub enum BatchQuery"),
        "the group module imports it instead of defining it again:\n{group}"
    );
    assert!(group.contains("use super::shared::BatchQuery;"), "{group}");
}

#[test]
fn without_a_hoist_a_one_group_type_stays_in_its_group() {
    let modules = generate(
        "no_hoist",
        &query_spec(),
        &["batch_query"],
        &AnalysisOptions::default(),
    );
    let shared = &modules.iter().find(|(n, _)| n == "shared").unwrap().1;
    assert!(!shared.contains("BatchQuery"), "{shared}");
}
