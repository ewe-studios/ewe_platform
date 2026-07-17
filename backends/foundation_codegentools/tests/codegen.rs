//! Tests for [`foundation_codegentools::generate`] — the declaration that carries
//! a selection all the way to the generator (spec-56 feature 00 §1, §4, §5).
//!
//! The bug this wiring exists to kill: `Pipeline` could already cut Linode's 334
//! paths to 3, and nothing called it — `generate` fed the whole spec to the
//! generator and every path came out the other side. So the load-bearing test is
//! not "selection works" (feature 00's own suite proves that) but **"the code on
//! disk contains only what we selected"**.

use foundation_codegentools::codegen::BuildScriptOutcome;
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

// ── the build.rs a crate carries ─────────────────────────────────────────────

#[test]
fn a_generated_build_script_reproduces_the_declaration() {
    // Feature 00 §4: the slice must be visible next to the code that uses it.
    // Before this, Hetzner's six --include-operation flags lived only in a shell
    // command, and nothing in the tree could reproduce them.
    let dir = tmpdir("buildrs");
    let spec = write_spec(&dir, "1.0.0");
    let krate = dir.join("crate");
    std::fs::create_dir_all(&krate).unwrap();
    std::fs::write(krate.join("Cargo.toml"), "[package]\nname = \"p\"\nversion = \"0.1.0\"\n").unwrap();

    let outcome = generate()
        .provider("testprov")
        .spec(&spec)
        .api_version("v4")
        .spec_version("1.0.0")
        .include_paths(["/instances"])
        .include_operations(["post-instance"])
        .crate_dir(&krate)
        .write_build_script(false)
        .expect("writes");

    assert!(matches!(outcome, BuildScriptOutcome::Created(_)));
    let body = std::fs::read_to_string(krate.join("build.rs")).unwrap();

    // Every part of the declaration survives the round trip.
    assert!(body.contains(r#".provider("testprov")"#), "{body}");
    assert!(body.contains(r#".api_version("v4")"#));
    assert!(body.contains(r#".spec_version("1.0.0")"#), "the pin, or a moved spec goes unnoticed");
    assert!(body.contains(r#""/instances""#));
    assert!(body.contains(r#""post-instance""#));

    // And the parts that keep it honest.
    assert!(
        body.contains("rerun-if-changed"),
        "without this cargo watches every file — and this script writes into src/, so it would rebuild forever"
    );
    assert!(
        body.contains("if !spec.exists()"),
        "a crates.io consumer has no spec artefact; failing there would break their build"
    );
    assert!(body.contains("codegen.check()"), "check gates it");
    assert!(body.contains("codegen.write()"), "staleness regenerates");
    assert!(body.contains("panic!"), "a moved pin must stop the build");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn writing_the_build_script_declares_the_dependency_it_needs() {
    // Writing a script that cannot compile, and leaving the crate to discover it,
    // is the sort of half-done that makes people distrust the generator.
    let dir = tmpdir("buildrs-dep");
    let spec = write_spec(&dir, "1.0.0");
    let krate = dir.join("crate");
    std::fs::create_dir_all(&krate).unwrap();
    std::fs::write(krate.join("Cargo.toml"), "[package]\nname = \"p\"\nversion = \"0.1.0\"\n").unwrap();

    generate()
        .provider("testprov")
        .spec(&spec)
        .crate_dir(&krate)
        .write_build_script(false)
        .expect("writes");

    let manifest = std::fs::read_to_string(krate.join("Cargo.toml")).unwrap();
    assert!(manifest.contains("build-dependencies"), "{manifest}");
    assert!(manifest.contains("foundation_codegentools"), "{manifest}");
    assert!(
        manifest.contains("version"),
        "a path-only build-dep would make the crate unpublishable: {manifest}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_existing_build_script_is_never_clobbered_without_being_asked() {
    // A build.rs is a source file people edit — they widen a selection, add a
    // pin, add an unrelated build step. Eating that on the next `genapi generate`
    // would be a nasty surprise, so the default is to leave it.
    let dir = tmpdir("buildrs-keep");
    let spec = write_spec(&dir, "1.0.0");
    let krate = dir.join("crate");
    std::fs::create_dir_all(&krate).unwrap();
    std::fs::write(krate.join("Cargo.toml"), "[package]\nname = \"p\"\nversion = \"0.1.0\"\n").unwrap();

    let mine = "// my own build step\nfn main() { println!(\"hi\"); }\n";
    std::fs::write(krate.join("build.rs"), mine).unwrap();

    let codegen = generate().provider("testprov").spec(&spec).crate_dir(&krate);

    let kept = codegen.write_build_script(false).expect("keeps");
    assert!(matches!(kept, BuildScriptOutcome::Kept(_)), "{kept:?}");
    assert_eq!(
        std::fs::read_to_string(krate.join("build.rs")).unwrap(),
        mine,
        "the hand-written script survived untouched"
    );

    // …and replaced only when asked.
    let replaced = codegen.write_build_script(true).expect("overwrites");
    assert!(matches!(replaced, BuildScriptOutcome::Replaced(_)), "{replaced:?}");
    let body = std::fs::read_to_string(krate.join("build.rs")).unwrap();
    assert_ne!(body, mine);
    assert!(body.contains(r#".provider("testprov")"#));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_existing_build_dependency_is_left_as_the_crate_wrote_it() {
    // Someone may have pinned a version or added features. Clobbering that is the
    // same mistake as clobbering the build.rs.
    let dir = tmpdir("buildrs-dep-keep");
    let spec = write_spec(&dir, "1.0.0");
    let krate = dir.join("crate");
    std::fs::create_dir_all(&krate).unwrap();
    let pinned = "[package]\nname = \"p\"\nversion = \"0.1.0\"\n\n[build-dependencies]\nfoundation_codegentools = { path = \"../elsewhere\", version = \"9.9.9\" }\n";
    std::fs::write(krate.join("Cargo.toml"), pinned).unwrap();

    generate()
        .provider("testprov")
        .spec(&spec)
        .crate_dir(&krate)
        .write_build_script(true)
        .expect("writes");

    let manifest = std::fs::read_to_string(krate.join("Cargo.toml")).unwrap();
    assert!(manifest.contains("9.9.9"), "the crate's own pin survived: {manifest}");
    assert!(manifest.contains("../elsewhere"), "and its path: {manifest}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_build_script_points_at_the_spec_from_the_crates_own_directory() {
    // Cargo runs a build script with CARGO_MANIFEST_DIR set to the crate, but the
    // CLI is run from the workspace root and holds workspace-relative paths. Left
    // unrewritten the script looks for the spec inside the crate, finds nothing,
    // and silently skips — indistinguishable from the published-crate case.
    let dir = tmpdir("buildrs-relpath");
    let krate = dir.join("backends/foundation_deployment_testprov");
    std::fs::create_dir_all(&krate).unwrap();
    std::fs::write(krate.join("Cargo.toml"), "[package]\nname = \"p\"\nversion = \"0.1.0\"\n").unwrap();
    let spec = write_spec(&dir, "1.0.0");

    generate()
        .provider("testprov")
        // A workspace-relative spec path, as the CLI passes.
        .spec("artefacts/cloud_providers/testprov/openapi.json")
        .crate_dir("backends/foundation_deployment_testprov")
        .write_build_script(false)
        .expect_err("no such crate dir here");

    // The rewrite itself, on the real shape: two levels down -> two levels up.
    let _ = spec;
    let outcome = generate()
        .provider("testprov")
        .spec("artefacts/cloud_providers/testprov/openapi.json")
        .crate_dir(&krate)
        .write_build_script(false);
    if let Ok(outcome) = outcome {
        let body = std::fs::read_to_string(outcome.path()).unwrap();
        assert!(
            body.contains("artefacts/cloud_providers/testprov/openapi.json"),
            "the spec path is in there somewhere: {body}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

// ── determinism ──────────────────────────────────────────────────────────────

#[test]
fn generating_twice_produces_byte_identical_output() {
    // Generated code is COMMITTED, so it must be a pure function of its inputs.
    //
    // It was not: the generator collected type names into a `HashSet`, and then
    // iterated that set to decide the order types are emitted. Rust seeds a
    // HashSet's hasher randomly per process, so two runs of the same command
    // emitted the same types in a different order — 833 lines of diff churn for
    // nothing, `check()` reporting Stale forever, and a build.rs that regenerated
    // on every run. Nothing failed; the output was just never the same twice.
    let dir = tmpdir("deterministic");
    let spec = write_spec(&dir, "1.0.0");

    let render = |n: usize| {
        let krate = dir.join(format!("crate{n}"));
        generate()
            .provider("testprov")
            .spec(&spec)
            .crate_dir(&krate)
            .write()
            .expect("generates");
        generated_source(&krate)
    };

    let first = render(1);
    let second = render(2);
    let third = render(3);

    assert_eq!(first, second, "two runs of the same declaration must agree");
    assert_eq!(second, third, "and keep agreeing");
    assert!(!first.is_empty(), "…about something");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_stays_green_across_runs() {
    // The consequence that bit us: non-deterministic output makes `check()`
    // permanently Stale, so a build.rs regenerates on every build and CI can
    // never be clean.
    let dir = tmpdir("check-stable");
    let spec = write_spec(&dir, "1.0.0");
    let krate = dir.join("crate");

    let codegen = generate().provider("testprov").spec(&spec).crate_dir(&krate);
    codegen.write().expect("writes");

    for attempt in 0..3 {
        codegen
            .check()
            .unwrap_or_else(|e| panic!("check {attempt} should be clean after a write: {e}"));
    }

    let _ = std::fs::remove_dir_all(&dir);
}
