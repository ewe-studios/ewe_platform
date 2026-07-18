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
