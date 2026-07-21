//! Tests for [`foundation_openapi::Pipeline`] — one declaration, two verbs
//! (spec-56 feature 00 §3–§5).
//!
//! The pin is the interesting part. Its whole job is to **fail** when a vendor's
//! spec moves, so the tests that matter are the ones proving it does — a pin that
//! cannot fail is decoration.

use foundation_openapi::{Pipeline, PipelineError, Selection};
use serde_json::json;

/// A bundled two-endpoint spec written to a temp dir, as a provider's raw file.
fn write_spec(dir: &std::path::Path, version: &str) -> std::path::PathBuf {
    let spec = json!({
        "openapi": "3.0.1",
        "info": { "title": "Test API", "version": version },
        "servers": [{ "url": "https://api.example.com" }],
        "paths": {
            "/instances": { "post": {
                "operationId": "post-instance",
                "tags": ["Instances"],
                "requestBody": { "content": { "application/json": {
                    "schema": { "type": "object", "properties": { "label": { "type": "string" } } } } } },
                "responses": { "200": { "content": { "application/json": {
                    "schema": { "type": "object", "properties": { "id": { "type": "integer" } } } } } } }
            }},
            "/kubernetes": { "get": {
                "operationId": "list-clusters",
                "tags": ["Kubernetes"],
                "responses": { "200": { "content": { "application/json": {
                    "schema": { "type": "object", "properties": { "name": { "type": "string" } } } } } } }
            }}
        },
        "components": { "schemas": {
            "Orphan": { "type": "object", "properties": { "x": { "type": "string" } } }
        }}
    });
    let path = dir.join("raw.json");
    std::fs::write(&path, serde_json::to_string_pretty(&spec).unwrap()).unwrap();
    path
}

fn tmpdir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ewe-pipeline-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ── the version pin ──────────────────────────────────────────────────────────

#[test]
fn a_matching_spec_version_resolves() {
    let dir = tmpdir("pin-ok");
    let spec = write_spec(&dir, "4.229.1");

    let resolved = Pipeline::new("test", &spec)
        .spec_version("4.229.1")
        .resolve()
        .expect("the pin matches");

    assert_eq!(resolved["info"]["version"], "4.229.1");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_moved_spec_version_is_fatal_and_names_both() {
    // The point of the pin: the vendor republished, and nobody chose to adopt it.
    let dir = tmpdir("pin-moved");
    let spec = write_spec(&dir, "4.230.0");

    let err = Pipeline::new("test", &spec)
        .spec_version("4.229.1")
        .resolve()
        .expect_err("a moved spec must not silently regenerate");

    match &err {
        PipelineError::SpecVersionMismatch { pinned, found } => {
            assert_eq!(pinned, "4.229.1");
            assert_eq!(found, "4.230.0");
        }
        other => panic!("expected SpecVersionMismatch, got {other:?}"),
    }

    // The message has to say what to do, not just that something is wrong.
    let msg = err.to_string();
    assert!(msg.contains("4.229.1") && msg.contains("4.230.0"), "names both: {msg}");
    assert!(msg.contains("bump the pin"), "says the fix: {msg}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_pin_means_no_check() {
    // Existing providers declare no pin and must keep working.
    let dir = tmpdir("pin-none");
    let spec = write_spec(&dir, "whatever");
    assert!(Pipeline::new("test", &spec).resolve().is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pin_against_a_spec_with_no_version_fails_rather_than_passing() {
    let dir = tmpdir("pin-missing");
    let path = dir.join("raw.json");
    std::fs::write(
        &path,
        serde_json::to_string(&json!({
            "openapi": "3.0.1",
            "info": { "title": "no version here" },
            "servers": [{"url": "https://x"}],
            "paths": {},
            "components": { "schemas": { "A": { "type": "object", "properties": {} } } }
        }))
        .unwrap(),
    )
    .unwrap();

    let err = Pipeline::new("test", &path)
        .spec_version("1.0.0")
        .resolve()
        .expect_err("cannot honour a pin against a spec that declares no version");
    assert!(matches!(err, PipelineError::SpecVersionMissing { .. }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_api_version_is_pinned_on_the_declaration() {
    let dir = tmpdir("api-version");
    let spec = write_spec(&dir, "1.0.0");
    let p = Pipeline::new("linode", &spec).api_version("v4");
    assert_eq!(p.pinned_api_version(), Some("v4"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ── the stages, in order ─────────────────────────────────────────────────────

#[test]
fn resolve_runs_select_then_canonicalize_then_prune() {
    let dir = tmpdir("stages");
    let spec = write_spec(&dir, "1.0.0");

    let resolved = Pipeline::new("test", &spec)
        .select(Selection::all().paths(["/instances"]))
        .resolve()
        .expect("resolves");

    // selected
    assert!(resolved["paths"]["/instances"].is_object());
    assert!(resolved["paths"]["/kubernetes"].is_null(), "unselected path is gone");

    // canonicalised — the inline response became a named type
    assert_eq!(
        resolved["paths"]["/instances"]["post"]["responses"]["200"]["content"]["application/json"]
            ["schema"]["$ref"],
        "#/components/schemas/PostInstanceResponse"
    );

    // pruned — the vendor's orphan went, and so did the unselected path's schema
    let schemas = resolved["components"]["schemas"].as_object().unwrap();
    assert!(schemas.contains_key("PostInstanceResponse"));
    assert!(schemas.contains_key("PostInstanceRequest"));
    assert!(!schemas.contains_key("Orphan"), "referenced by nothing");
    assert!(
        !schemas.keys().any(|k| k.contains("Cluster")),
        "the unselected path's types are gone: {:?}",
        schemas.keys().collect::<Vec<_>>()
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resolve_output_is_canonical_and_self_contained() {
    let dir = tmpdir("invariants");
    let spec = write_spec(&dir, "1.0.0");

    let resolved = Pipeline::new("test", &spec)
        .select(Selection::all().paths(["/instances"]))
        .resolve()
        .expect("resolves");

    assert_eq!(foundation_openapi::validate_canonical(&resolved), Ok(()));
    assert_eq!(foundation_openapi::dangling_refs(&resolved), Vec::<String>::new());
    assert_eq!(foundation_openapi::unreachable_components(&resolved), Vec::<String>::new());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_spec_says_which_file() {
    let err = Pipeline::new("test", "/nowhere/at/all.json")
        .resolve()
        .expect_err("no such file");
    assert!(err.to_string().contains("/nowhere/at/all.json"), "{err}");
}

// ── check() and write() ──────────────────────────────────────────────────────

#[test]
fn write_then_check_agrees() {
    let dir = tmpdir("write-check");
    let spec = write_spec(&dir, "1.0.0");
    let out = dir.join("out");

    let p = Pipeline::new("test", &spec).select(Selection::all().paths(["/instances"]));

    let written = p.write(&out).expect("writes");
    assert!(written.exists());
    p.check(&out).expect("what was just written matches the declaration");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_fails_when_the_declaration_changes_under_the_output() {
    // The staleness guard: someone edits the selection and forgets to regenerate.
    let dir = tmpdir("stale");
    let spec = write_spec(&dir, "1.0.0");
    let out = dir.join("out");

    Pipeline::new("test", &spec)
        .select(Selection::all().paths(["/instances"]))
        .write(&out)
        .expect("writes");

    let err = Pipeline::new("test", &spec)
        .select(Selection::all().paths(["/instances", "/kubernetes"])) // widened
        .check(&out)
        .expect_err("the committed output no longer matches");

    assert!(matches!(err, PipelineError::Stale { .. }), "{err:?}");
    assert!(err.to_string().contains("regenerate"), "says the fix: {err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_fails_when_nothing_has_been_generated_yet() {
    let dir = tmpdir("never-written");
    let spec = write_spec(&dir, "1.0.0");

    let err = Pipeline::new("test", &spec)
        .check(&dir.join("nonexistent"))
        .expect_err("nothing to compare against");
    assert!(matches!(err, PipelineError::Stale { .. }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_does_not_write() {
    // check() is the build-script/CI verb — these crates are publishable, and a
    // build script writing into src/ would break a crates.io consumer.
    let dir = tmpdir("check-readonly");
    let spec = write_spec(&dir, "1.0.0");
    let out = dir.join("out");

    let _ = Pipeline::new("test", &spec).check(&out);
    assert!(!out.exists(), "check() must not create anything");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_catches_a_moved_spec_too() {
    // A build that keeps compiling against a spec that moved is the failure the
    // pin exists to prevent — check() must enforce it, not just write().
    let dir = tmpdir("check-pin");
    let spec = write_spec(&dir, "1.0.0");
    let out = dir.join("out");

    Pipeline::new("test", &spec)
        .spec_version("1.0.0")
        .write(&out)
        .expect("writes");

    // The vendor republishes.
    write_spec(&dir, "2.0.0");

    let err = Pipeline::new("test", &spec)
        .spec_version("1.0.0")
        .check(&out)
        .expect_err("the pin no longer holds");
    assert!(matches!(err, PipelineError::SpecVersionMismatch { .. }), "{err:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The declaration feature 03 will actually carry, run against Linode's real
/// 9.3 MB / 334-path spec.
///
/// Skips when the owner-supplied clone is absent.
#[test]
fn the_real_linode_declaration() {
    const SPEC: &str = "/home/darkvoid/Boxxed/@formulas/src.rust/src.cloud_providers/\
                        src.linode/linode-api-openapi/openapi.json";
    if !std::path::Path::new(SPEC).exists() {
        eprintln!("SKIP: Linode spec not present");
        return;
    }

    let pipeline = Pipeline::new("linode", SPEC)
        .api_version("v4")
        .spec_version("4.229.1")
        .base_url("https://api.linode.com")
        .select(Selection::all().paths([
            "/{apiVersion}/linode/instances",
            "/{apiVersion}/linode/instances/{linodeId}",
            "/{apiVersion}/profile/sshkeys",
        ]));

    let resolved = pipeline.resolve().expect("the real spec resolves");

    assert_eq!(resolved["paths"].as_object().unwrap().len(), 3, "3 of 334");
    assert_eq!(foundation_openapi::validate_canonical(&resolved), Ok(()));
    assert_eq!(foundation_openapi::dangling_refs(&resolved), Vec::<String>::new());

    let schemas = foundation_openapi::component_count(&resolved, "schemas");
    assert!(schemas < 60, "a bounded surface, got {schemas}");
    eprintln!("  linode: 334 paths -> 3, components/schemas -> {schemas}");

    // And the pin has teeth against the real document.
    let wrong = Pipeline::new("linode", SPEC).spec_version("0.0.0-not-real");
    assert!(matches!(
        wrong.resolve(),
        Err(PipelineError::SpecVersionMismatch { .. })
    ));
}

// ── whose broken ref is it? ──────────────────────────────────────────────────

#[test]
fn a_vendors_own_dangling_ref_is_reported_not_fatal() {
    // Cloudflare's spec references 57 schemas it never bundles. Refusing to
    // generate over the vendor's bug would mean we cannot support them at all —
    // and the old path generated it fine (those fields come out untyped).
    let dir = tmpdir("vendor-dangling");
    let path = dir.join("raw.json");
    std::fs::write(
        &path,
        serde_json::to_string(&json!({
            "openapi": "3.0.1",
            "info": { "title": "Sloppy", "version": "1.0.0" },
            "servers": [{ "url": "https://x" }],
            "paths": { "/a": { "post": {
                "operationId": "post-a",
                // One ref the vendor bundles, one they forgot — the shape
                // Cloudflare actually ships.
                "requestBody": { "content": { "application/json": {
                    "schema": { "$ref": "#/components/schemas/Present" } } } },
                "responses": { "200": { "content": { "application/json": {
                    "schema": { "$ref": "#/components/schemas/NeverBundled" } } } } }
            }}},
            "components": { "schemas": {
                "Present": { "type": "object", "properties": { "x": { "type": "string" } } }
            }}
        }))
        .unwrap(),
    )
    .unwrap();

    let resolved = Pipeline::new("sloppy", &path)
        .resolve()
        .expect("a vendor's own dangling ref must not stop generation");

    // It is still dangling — we did not invent a schema to paper over it.
    assert_eq!(
        foundation_openapi::dangling_refs(&resolved),
        vec!["#/components/schemas/NeverBundled".to_string()]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_ref_we_break_ourselves_is_fatal() {
    // The invariant that is ours to hold: a ref that resolved in the vendor's
    // document must still resolve in ours. This is what a pruning bug looks like.
    let dir = tmpdir("we-broke-it");
    let path = dir.join("raw.json");
    std::fs::write(
        &path,
        serde_json::to_string(&json!({
            "openapi": "3.0.1",
            "info": { "title": "Fine", "version": "1.0.0" },
            "servers": [{ "url": "https://x" }],
            "paths": { "/a": { "get": {
                "operationId": "get-a",
                "responses": { "200": { "content": { "application/json": {
                    "schema": { "$ref": "#/components/schemas/Thing" } } } } }
            }}},
            "components": { "schemas": {
                "Thing": { "type": "object", "properties": {
                    "other": { "$ref": "#/components/schemas/Other" } } },
                "Other": { "type": "object", "properties": { "x": { "type": "string" } } }
            }}
        }))
        .unwrap(),
    )
    .unwrap();

    // Nothing dangles in the source, and nothing must dangle after we prune: the
    // closure has to follow Thing -> Other.
    let resolved = Pipeline::new("fine", &path).resolve().expect("resolves");
    assert_eq!(foundation_openapi::dangling_refs(&resolved), Vec::<String>::new());
    assert!(
        resolved["components"]["schemas"]["Other"].is_object(),
        "a transitively-referenced schema survives pruning"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
