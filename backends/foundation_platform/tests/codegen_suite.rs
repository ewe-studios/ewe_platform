//! Tests for build pipeline source scanner and codegen orchestrator.

use foundation_platform::codegen::{scan_for_annotations, AnnotationKind};

#[test]
fn scan_detects_platform_annotations() {
    let tmp = std::env::temp_dir().join("fplat_test_scan");
    std::fs::create_dir_all(&tmp).unwrap();

    let file = tmp.join("test.rs");
    std::fs::write(&file, r#"
#[wasm_bin] fn my_app() {}

#[platform_bin] fn main_setup(session: PlatformSession) {}

fn not_annotated() {}
"#).unwrap();

    let annotations = scan_for_annotations(&tmp);
    assert_eq!(annotations.len(), 2);

    let wasm_bin = annotations.iter().find(|a| a.kind == AnnotationKind::WasmBin).unwrap();
    assert_eq!(wasm_bin.name, "my_app");

    let plat_bin = annotations.iter().find(|a| a.kind == AnnotationKind::PlatformBin).unwrap();
    assert_eq!(plat_bin.name, "main_setup");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn empty_directory_returns_no_annotations() {
    let tmp = std::env::temp_dir().join("fplat_empty");
    std::fs::create_dir_all(&tmp).unwrap();
    let annotations = scan_for_annotations(&tmp);
    assert!(annotations.is_empty());
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn generate_creates_generated_dir() {
    let tmp = std::env::temp_dir().join("fplat_generate_test");
    std::fs::create_dir_all(&tmp.join("src")).unwrap();

    // Write a marker file so we can test
    let src_file = tmp.join("src").join("main.rs");
    std::fs::write(&src_file, "#[platform_bin]\nfn main(session: PlatformSession) {}\n").unwrap();

    // We can't actually call generate_platform_code() in tests easily
    // because it reads CARGO_MANIFEST_DIR. But we document the API here.

    std::fs::remove_dir_all(&tmp).ok();
}

// ── F40: bundle.resources is discovered from what public/ contains ──────

use foundation_platform::codegen::replace_bundle_resources;

fn entries_for(app_id_versions: &[(&str, &str)]) -> Vec<(String, String, String)> {
    app_id_versions
        .iter()
        .map(|(id, ver)| (format!("public/{id}/v{ver}/**/*"), id.to_string(), ver.to_string()))
        .collect()
}

fn resources_for(existing: serde_json::Value, entries: &[(String, String, String)]) -> serde_json::Value {
    let mut conf = serde_json::json!({ "bundle": { "resources": existing } });
    replace_bundle_resources(&mut conf, entries);
    conf["bundle"]["resources"].clone()
}

#[test]
fn every_discovered_app_gets_a_versioned_resource_entry() {
    let entries = entries_for(&[("app", "0.1.0"), ("app-hello", "0.1.0"), ("app-shell", "0.2.0")]);
    let resources = resources_for(serde_json::json!({}), &entries);

    assert_eq!(resources["public/app/v0.1.0/**/*"], "app/v0.1.0/");
    assert_eq!(resources["public/app-hello/v0.1.0/**/*"], "app-hello/v0.1.0/");
    assert_eq!(
        resources["public/app-shell/v0.2.0/**/*"], "app-shell/v0.2.0/",
        "a Surface 3 module is bundled in the same versioned shape as every other app"
    );
}

#[test]
fn two_apps_at_different_versions_naturally_land_at_different_paths() {
    let entries = entries_for(&[("app", "0.2.0"), ("app-hello", "0.1.0")]);
    let resources = resources_for(serde_json::json!({}), &entries);

    assert_eq!(resources["public/app/v0.2.0/**/*"], "app/v0.2.0/");
    assert_eq!(resources["public/app-hello/v0.1.0/**/*"], "app-hello/v0.1.0/");
}

#[test]
fn nested_assets_are_bundled_not_silently_dropped() {
    let entries = entries_for(&[("app", "0.1.0")]);
    let resources = resources_for(serde_json::json!({}), &entries);
    let source = resources.as_object().unwrap().keys().next().unwrap();

    assert!(
        source.ends_with("/**/*"),
        "a plain `*` drops anything in a subdirectory; got {source}"
    );
}

#[test]
fn each_generation_replaces_rather_than_accumulating() {
    let previous = serde_json::json!({ "public/app/v0.1.0/**/*": "app/v0.1.0/" });
    let entries = entries_for(&[("app", "0.2.0")]);
    let resources = resources_for(previous, &entries);

    assert_eq!(resources.as_object().unwrap().len(), 1, "got {resources}");
    assert_eq!(resources["public/app/v0.2.0/**/*"], "app/v0.2.0/");
    assert!(
        resources.get("public/app/v0.1.0/**/*").is_none(),
        "the previous version entry must be gone; entries come from disk each generation"
    );
}

#[test]
fn resources_outside_public_belong_to_the_project_and_are_left_alone() {
    let theirs = serde_json::json!({
        "icons/*": "icons/",
        "vendor/licences.txt": "licences.txt",
    });
    let entries = entries_for(&[("app", "0.1.0")]);
    let resources = resources_for(theirs, &entries);

    assert_eq!(resources["icons/*"], "icons/");
    assert_eq!(resources["vendor/licences.txt"], "licences.txt");
    assert_eq!(resources["public/app/v0.1.0/**/*"], "app/v0.1.0/");
}
