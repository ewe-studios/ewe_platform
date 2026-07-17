//! Tests for `genapi normalize` — the raw → canonical artefact command
//! (spec-56 decision 05, feature 00 §0).
//!
//! The registry's own classification is the thing most likely to be wrong: it is a
//! human judgement about a vendor's spec ("DigitalOcean is ref-based, so it needs
//! nothing") and it was in fact wrong — DO has 7814 refs *and* 58 inline schemas.
//! The validator caught it, which is why these tests care most about the failure
//! paths.

#![cfg(feature = "cli")]

use foundation_codegentools::cli::normalize::{
    normalize_provider, provider_spec, Normalizer, ProviderSpec, PROVIDER_SPECS,
};
use serde_json::json;

fn tmpdir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ewe-norm-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write a raw spec where the registry expects one, under `root`.
fn write_raw(root: &std::path::Path, rel: &str, spec: &serde_json::Value) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, serde_json::to_string_pretty(spec).unwrap()).unwrap();
}

// ── the registry ─────────────────────────────────────────────────────────────

#[test]
fn the_registry_holds_the_three_providers() {
    for name in ["hetzner", "linode", "digitalocean"] {
        assert!(provider_spec(name).is_some(), "{name} should be registered");
    }
    assert!(provider_spec("nope").is_none());
}

#[test]
fn every_registered_raw_path_lives_under_the_raw_directory() {
    // Decision 05: raw specs are committed under artefacts/cloud_providers/raw/,
    // so normalization is deterministic and offline.
    for spec in PROVIDER_SPECS {
        assert!(
            spec.raw.starts_with("artefacts/cloud_providers/raw/"),
            "{}: raw specs live in raw/, got {}",
            spec.name,
            spec.raw
        );
        assert!(!spec.source.is_empty(), "{}: provenance is not optional", spec.name);
    }
}

#[test]
fn digitalocean_hoists_rather_than_passing_through() {
    // Pinning the correction: ref-heavy is not the same as canonical. DO has 7814
    // refs and 58 inline operation schemas; classifying it ValidateOnly made
    // `normalize` fail, which is the validator earning its keep.
    assert_eq!(
        provider_spec("digitalocean").unwrap().normalizer,
        Normalizer::HoistInline
    );
}

// ── normalize_provider ───────────────────────────────────────────────────────

fn bundled_spec() -> serde_json::Value {
    json!({
        "openapi": "3.0.1",
        "info": { "title": "Test", "version": "9.9.9" },
        "paths": { "/things": { "post": {
            "operationId": "post-thing",
            "requestBody": { "content": { "application/json": {
                "schema": { "type": "object", "properties": { "name": { "type": "string" } } } } } },
            "responses": { "200": { "content": { "application/json": {
                "schema": { "type": "object", "properties": { "id": { "type": "integer" } } } } } } }
        }}},
        "components": { "schemas": {} }
    })
}

fn test_provider() -> ProviderSpec {
    ProviderSpec {
        name: "testprov",
        raw: "artefacts/cloud_providers/raw/testprov.json",
        source: "https://example.com/spec.json",
        base_url: "https://api.example.com",
        normalizer: Normalizer::HoistInline,
    }
}

#[test]
fn normalizes_a_bundled_spec_and_writes_artefact_plus_manifest() {
    let root = tmpdir("bundled");
    let out = root.join("artefacts/cloud_providers");
    write_raw(&root, "artefacts/cloud_providers/raw/testprov.json", &bundled_spec());

    let report = normalize_provider(&test_provider(), &root, &out).expect("normalizes");

    assert_eq!(report.inline_before, 2, "request + response were inline");
    assert_eq!(report.hoisted, 2);
    assert_eq!(report.spec_version.as_deref(), Some("9.9.9"));

    // The artefact is canonical.
    let canonical: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("testprov/openapi.json")).unwrap())
            .unwrap();
    assert_eq!(foundation_openapi::validate_canonical(&canonical), Ok(()));
    assert_eq!(
        canonical["paths"]["/things"]["post"]["responses"]["200"]["content"]["application/json"]
            ["schema"]["$ref"],
        "#/components/schemas/PostThingResponse"
    );

    // The manifest carries what the pin and provenance need.
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("testprov/_manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["spec_version"], "9.9.9", "what feature 00's pin checks");
    assert_eq!(manifest["openapi"], "3.0.1");
    assert_eq!(manifest["source"], "https://example.com/spec.json");
    assert_eq!(manifest["hoisted"], 2);
    assert!(
        manifest["transforms"]
            .as_array()
            .unwrap()
            .contains(&json!("canonicalize_operations")),
        "otherwise 'why does this differ from the vendor's?' is unanswerable"
    );
    assert!(manifest["raw_sha256"].as_str().unwrap().len() == 64, "detects a moved vendor spec");
    assert!(manifest["canonical_sha256"].as_str().unwrap().len() == 64, "detects hand-editing");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_wrong_classification_fails_loudly_rather_than_writing_a_broken_artefact() {
    // This is the DigitalOcean mistake, reproduced: claim a spec needs nothing
    // when it has inline schemas. Without validation it would write an artefact
    // that generates anonymous blobs — quietly.
    let root = tmpdir("misclassified");
    let out = root.join("artefacts/cloud_providers");
    write_raw(&root, "artefacts/cloud_providers/raw/testprov.json", &bundled_spec());

    let mut wrong = test_provider();
    wrong.normalizer = Normalizer::ValidateOnly;

    let err = normalize_provider(&wrong, &root, &out).expect_err("must not pass");
    let msg = err.to_string();
    assert!(msg.contains("not canonical"), "{msg}");
    assert!(msg.contains("inline object schema"), "says what is wrong: {msg}");
    assert!(
        !out.join("testprov/openapi.json").exists(),
        "a failed normalize must not leave an artefact behind"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_missing_raw_spec_names_the_file() {
    let root = tmpdir("missing");
    let out = root.join("artefacts/cloud_providers");
    let err = normalize_provider(&test_provider(), &root, &out).expect_err("no raw spec");
    assert!(err.to_string().contains("raw/testprov.json"), "{err}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_spec_that_is_not_json_says_so() {
    let root = tmpdir("notjson");
    let out = root.join("artefacts/cloud_providers");
    let path = root.join("artefacts/cloud_providers/raw/testprov.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "openapi: 3.0.1\npaths: {}\n").unwrap(); // YAML, not JSON

    let err = normalize_provider(&test_provider(), &root, &out).expect_err("not JSON");
    assert!(err.to_string().contains("not JSON"), "{err}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn normalizing_is_deterministic() {
    // The raw spec is committed precisely so that re-running is reproducible: the
    // same input must give a byte-identical artefact, or `check()` would flap.
    let root = tmpdir("deterministic");
    let out = root.join("artefacts/cloud_providers");
    write_raw(&root, "artefacts/cloud_providers/raw/testprov.json", &bundled_spec());

    let first = normalize_provider(&test_provider(), &root, &out).expect("first");
    let a = std::fs::read_to_string(out.join("testprov/openapi.json")).unwrap();
    let second = normalize_provider(&test_provider(), &root, &out).expect("second");
    let b = std::fs::read_to_string(out.join("testprov/openapi.json")).unwrap();

    assert_eq!(a, b, "same input, same artefact");
    assert_eq!(first.hoisted, second.hoisted);
    let _ = std::fs::remove_dir_all(&root);
}
