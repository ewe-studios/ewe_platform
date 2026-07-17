//! Tests for [`foundation_codegentools::generate`] — the declaration that carries
//! a selection all the way to the generator (spec-56 feature 00 §1, §4, §5).
//!
//! The bug this wiring exists to kill: `Pipeline` could already cut Linode's 334
//! paths to 3, and nothing called it — `generate` fed the whole spec to the
//! generator and every path came out the other side. So the load-bearing test is
//! not "selection works" (feature 00's own suite proves that) but **"the code on
//! disk contains only what we selected"**.

use foundation_codegentools::{generate, CodegenError};
use serde_json::json;

fn tmpdir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ewe-codegen-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A two-endpoint spec: one we want, one we do not.
fn write_spec(dir: &std::path::Path, version: &str) -> std::path::PathBuf {
    let spec = json!({
        "openapi": "3.0.1",
        "info": { "title": "Test API", "version": version },
        "servers": [{ "url": "https://api.example.com" }],
        "paths": {
            "/servers": { "get": {
                "operationId": "list-servers",
                "tags": ["Servers"],
                "responses": { "200": { "content": { "application/json": {
                    "schema": { "type": "object", "properties": { "id": { "type": "integer" } } } } } } }
            }},
            "/billing/invoices": { "get": {
                "operationId": "list-invoices",
                "tags": ["Billing"],
                "responses": { "200": { "content": { "application/json": {
                    "schema": { "type": "object", "properties": { "amount": { "type": "integer" } } } } } } }
            }}
        },
        "components": { "schemas": {} }
    });
    let path = dir.join("openapi.json");
    std::fs::write(&path, serde_json::to_string_pretty(&spec).unwrap()).unwrap();
    path
}

/// Every generated .rs file's contents, concatenated.
fn generated_source(crate_dir: &std::path::Path) -> String {
    fn walk(dir: &std::path::Path, out: &mut String) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if let Ok(body) = std::fs::read_to_string(&path) {
                out.push_str(&body);
            }
        }
    }
    let mut out = String::new();
    walk(&crate_dir.join("src/generated"), &mut out);
    out
}

// ── the selection reaches the generator ──────────────────────────────────────

#[test]
fn only_the_selected_paths_are_generated() {
    // This is the whole feature: code for the endpoint we asked for, and NO code
    // for the one we did not. Asserting the absence is the half that matters —
    // "we generated all 334 paths anyway" passes any test that only checks the
    // selected path is present.
    let dir = tmpdir("selected");
    let spec = write_spec(&dir, "1.0.0");
    let krate = dir.join("crate");

    let report = generate()
        .provider("testprov")
        .spec(&spec)
        .include_paths(["/servers"])
        .crate_dir(&krate)
        .write()
        .expect("generates");

    assert_eq!(report.paths_selected, 1, "1 of 2");
    assert!(report.files > 0, "something was written");
    assert_eq!(report.generated_dir, krate.join("src/generated"));

    let source = generated_source(&krate);
    assert!(!source.is_empty(), "generated something");
    assert!(
        source.contains("servers") || source.to_lowercase().contains("server"),
        "the selected endpoint is generated"
    );
    assert!(
        !source.contains("invoice") && !source.contains("Invoice"),
        "the unselected endpoint must not appear anywhere in the generated tree"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn selecting_by_tag_reaches_the_generator_too() {
    let dir = tmpdir("by-tag");
    let spec = write_spec(&dir, "1.0.0");
    let krate = dir.join("crate");

    let report = generate()
        .provider("testprov")
        .spec(&spec)
        .include_tags(["Billing"])
        .crate_dir(&krate)
        .write()
        .expect("generates");

    assert_eq!(report.paths_selected, 1);
    let source = generated_source(&krate);
    assert!(
        !source.contains("list_servers") && !source.contains("ListServers"),
        "the untagged endpoint is gone"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_selection_generates_everything() {
    // Existing providers (cloudflare, gcp, stripe) declare no selection and must
    // keep generating exactly as before.
    let dir = tmpdir("all");
    let spec = write_spec(&dir, "1.0.0");
    let krate = dir.join("crate");

    let report = generate()
        .provider("testprov")
        .spec(&spec)
        .crate_dir(&krate)
        .write()
        .expect("generates");

    assert_eq!(report.paths_selected, 2, "both paths");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_generated_types_are_named_because_the_spec_was_canonicalised_first() {
    // The declaration runs canonicalisation, so a bundled spec's inline response
    // becomes a named type rather than an anonymous blob. Without this, Linode's
    // endpoints generate nothing usable.
    let dir = tmpdir("named");
    let spec = write_spec(&dir, "1.0.0");
    let krate = dir.join("crate");

    generate()
        .provider("testprov")
        .spec(&spec)
        .include_paths(["/servers"])
        .crate_dir(&krate)
        .write()
        .expect("generates");

    let source = generated_source(&krate);
    assert!(
        source.contains("ListServersResponse"),
        "the hoisted response type is generated by name; got:\n{}",
        &source[..source.len().min(600)]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ── the pins ─────────────────────────────────────────────────────────────────

#[test]
fn a_moved_spec_fails_the_run_and_writes_nothing() {
    let dir = tmpdir("pin");
    let spec = write_spec(&dir, "2.0.0");
    let krate = dir.join("crate");

    let err = generate()
        .provider("testprov")
        .spec(&spec)
        .spec_version("1.0.0")
        .crate_dir(&krate)
        .write()
        .expect_err("the pin does not match");

    assert!(matches!(err, CodegenError::Pipeline(_)), "{err:?}");
    assert!(err.to_string().contains("1.0.0") && err.to_string().contains("2.0.0"));
    assert!(
        !krate.join("src/generated").exists(),
        "a failed pin must not leave generated code behind"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ── check() ──────────────────────────────────────────────────────────────────

#[test]
fn check_passes_against_what_write_just_produced() {
    let dir = tmpdir("check-ok");
    let spec = write_spec(&dir, "1.0.0");
    let krate = dir.join("crate");

    let codegen = generate()
        .provider("testprov")
        .spec(&spec)
        .include_paths(["/servers"])
        .crate_dir(&krate);

    codegen.write().expect("writes");
    codegen.check().expect("what was just written matches");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_fails_and_names_the_files_when_the_selection_widens() {
    // Someone adds an endpoint to the declaration and forgets to regenerate. The
    // build must say so, and say which files.
    let dir = tmpdir("check-stale");
    let spec = write_spec(&dir, "1.0.0");
    let krate = dir.join("crate");

    generate()
        .provider("testprov")
        .spec(&spec)
        .include_paths(["/servers"])
        .crate_dir(&krate)
        .write()
        .expect("writes");

    let err = generate()
        .provider("testprov")
        .spec(&spec)
        .include_paths(["/servers", "/billing/invoices"]) // widened
        .crate_dir(&krate)
        .check()
        .expect_err("the committed output is now stale");

    match &err {
        CodegenError::Stale { added, changed, .. } => {
            assert!(
                !added.is_empty() || !changed.is_empty(),
                "the new endpoint shows up as added or changed files"
            );
        }
        other => panic!("expected Stale, got {other:?}"),
    }
    assert!(err.to_string().contains("regenerate"), "says the fix: {err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_reports_never_generated_as_stale_rather_than_an_io_error() {
    let dir = tmpdir("check-empty");
    let spec = write_spec(&dir, "1.0.0");

    let err = generate()
        .provider("testprov")
        .spec(&spec)
        .crate_dir(dir.join("nothing-here"))
        .check()
        .expect_err("nothing has been generated");
    assert!(matches!(err, CodegenError::Stale { .. }), "{err:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_writes_nothing_at_all() {
    // check() is the build.rs/CI verb. These crates are publishable, so it must
    // not touch src/ — nor the crate's Cargo.toml, which the generator edits to
    // add feature flags.
    let dir = tmpdir("check-readonly");
    let spec = write_spec(&dir, "1.0.0");
    let krate = dir.join("crate");
    std::fs::create_dir_all(&krate).unwrap();
    let manifest = krate.join("Cargo.toml");
    let original = "[package]\nname = \"testprov\"\nversion = \"0.1.0\"\n\n[features]\n";
    std::fs::write(&manifest, original).unwrap();

    let _ = generate()
        .provider("testprov")
        .spec(&spec)
        .include_paths(["/servers"])
        .crate_dir(&krate)
        .check();

    assert!(!krate.join("src").exists(), "check() created no source");
    assert_eq!(
        std::fs::read_to_string(&manifest).unwrap(),
        original,
        "check() did not touch the manifest either"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_catches_a_moved_spec_too() {
    // A build that keeps compiling against a spec that moved is the failure the
    // pin exists to prevent, so check() must enforce it — not just write().
    let dir = tmpdir("check-pin");
    let spec = write_spec(&dir, "1.0.0");
    let krate = dir.join("crate");

    generate()
        .provider("testprov")
        .spec(&spec)
        .spec_version("1.0.0")
        .crate_dir(&krate)
        .write()
        .expect("writes");

    write_spec(&dir, "2.0.0"); // the vendor republishes

    let err = generate()
        .provider("testprov")
        .spec(&spec)
        .spec_version("1.0.0")
        .crate_dir(&krate)
        .check()
        .expect_err("the pin no longer holds");
    assert!(matches!(err, CodegenError::Pipeline(_)), "{err:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── the declaration itself ───────────────────────────────────────────────────

#[test]
fn an_incomplete_declaration_names_what_is_missing() {
    let err = generate().spec("/some/spec.json").resolve().expect_err("no provider");
    assert!(matches!(err, CodegenError::Incomplete { missing: "provider" }), "{err:?}");

    let err = generate().provider("x").resolve().expect_err("no spec");
    assert!(matches!(err, CodegenError::Incomplete { missing: "spec" }), "{err:?}");

    let dir = tmpdir("no-dest");
    let spec = write_spec(&dir, "1.0.0");
    let err = generate()
        .provider("x")
        .spec(&spec)
        .write()
        .expect_err("nowhere to write");
    assert!(matches!(err, CodegenError::Incomplete { missing: "crate_dir" }), "{err:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The declaration feature 03 will carry, against Linode's real 9.3 MB spec.
///
/// Skips when the owner-supplied clone is absent.
#[test]
fn the_real_linode_slice_generates_a_bounded_crate() {
    const SPEC: &str = "/home/darkvoid/Boxxed/@formulas/src.rust/src.cloud_providers/\
                        src.linode/linode-api-openapi/openapi.json";
    if !std::path::Path::new(SPEC).exists() {
        eprintln!("SKIP: Linode spec not present");
        return;
    }

    let dir = tmpdir("linode");
    let krate = dir.join("foundation_deployment_linode");

    let report = generate()
        .provider("linode")
        .spec(SPEC)
        .api_version("v4")
        .spec_version("4.229.1")
        .include_paths([
            "/{apiVersion}/linode/instances",
            "/{apiVersion}/linode/instances/{linodeId}",
            "/{apiVersion}/profile/sshkeys",
        ])
        .crate_dir(&krate)
        .write()
        .expect("the real spec generates");

    assert_eq!(report.paths_selected, 3, "3 of 334");
    assert!(report.schemas < 60, "a bounded surface, got {}", report.schemas);

    // The payoff, stated as the thing that would have caught the old behaviour:
    // 334 paths of Linode would not fit in a handful of files.
    let source = generated_source(&krate);
    assert!(
        !source.contains("Nodebalancer") && !source.contains("nodebalancer"),
        "an unselected Linode API must not be generated"
    );
    eprintln!(
        "  linode: 334 paths -> {}, {} schemas, {} files",
        report.paths_selected, report.schemas, report.files
    );

    let _ = std::fs::remove_dir_all(&dir);
}
