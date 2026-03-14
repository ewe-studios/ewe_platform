//! Unit tests for `CrateMetadata::from_cargo_toml()` in `foundation_codegen`.
//!
//! Tests valid parsing (lib, bin, custom path), error conditions
//! (missing file, missing fields), and edge cases (missing version).

use std::fs;
use std::path::Path;

use foundation_codegen::{CodegenError, CrateMetadata};
use tempfile::TempDir;

/// Helper: create a minimal crate directory with Cargo.toml and either lib.rs or main.rs.
fn create_crate(dir: &Path, cargo_toml: &str, has_lib: bool) {
    fs::write(dir.join("Cargo.toml"), cargo_toml).unwrap();
    let src = dir.join("src");
    fs::create_dir_all(&src).unwrap();
    if has_lib {
        fs::write(src.join("lib.rs"), "// lib").unwrap();
    } else {
        fs::write(src.join("main.rs"), "fn main() {}").unwrap();
    }
}

// -- Valid input: standard crate layouts --

#[test]
fn parses_standard_lib_crate() {
    let tmp = TempDir::new().unwrap();
    create_crate(
        tmp.path(),
        "[package]\nname = \"my_crate\"\nversion = \"1.2.3\"\n",
        true,
    );

    let meta = CrateMetadata::from_cargo_toml(&tmp.path().join("Cargo.toml")).unwrap();
    assert_eq!(meta.name, "my_crate");
    assert_eq!(meta.version, "1.2.3");
    assert!(meta.is_lib);
    assert!(meta.entry_point.ends_with("lib.rs"));
}

#[test]
fn parses_bin_crate() {
    let tmp = TempDir::new().unwrap();
    create_crate(
        tmp.path(),
        "[package]\nname = \"my_bin\"\nversion = \"0.1.0\"\n",
        false,
    );

    let meta = CrateMetadata::from_cargo_toml(&tmp.path().join("Cargo.toml")).unwrap();
    assert_eq!(meta.name, "my_bin");
    assert!(!meta.is_lib);
    assert!(meta.entry_point.ends_with("main.rs"));
}

#[test]
fn parses_custom_lib_path() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("my_lib.rs"), "// custom lib").unwrap();
    fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"custom_lib\"\nversion = \"0.1.0\"\n\n[lib]\npath = \"src/my_lib.rs\"\n",
    )
    .unwrap();

    let meta = CrateMetadata::from_cargo_toml(&tmp.path().join("Cargo.toml")).unwrap();
    assert_eq!(meta.name, "custom_lib");
    assert!(meta.is_lib);
    assert!(meta.entry_point.ends_with("my_lib.rs"));
}

// -- Invalid input: missing required components --

#[test]
fn error_when_cargo_toml_missing() {
    let tmp = TempDir::new().unwrap();
    let result = CrateMetadata::from_cargo_toml(&tmp.path().join("Cargo.toml"));
    assert!(result.is_err(), "should fail when file does not exist");
}

#[test]
fn error_when_package_section_missing() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("lib.rs"), "").unwrap();
    fs::write(tmp.path().join("Cargo.toml"), "[dependencies]\n").unwrap();

    let result = CrateMetadata::from_cargo_toml(&tmp.path().join("Cargo.toml"));
    assert!(
        matches!(result, Err(CodegenError::MissingPackageSection(_))),
        "should produce MissingPackageSection"
    );
}

#[test]
fn error_when_package_name_missing() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("lib.rs"), "").unwrap();
    fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nversion = \"1.0\"\n",
    )
    .unwrap();

    let result = CrateMetadata::from_cargo_toml(&tmp.path().join("Cargo.toml"));
    assert!(
        matches!(result, Err(CodegenError::MissingPackageName(_))),
        "should produce MissingPackageName"
    );
}

#[test]
fn error_when_src_dir_missing() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\nversion = \"1.0\"\n",
    )
    .unwrap();

    let result = CrateMetadata::from_cargo_toml(&tmp.path().join("Cargo.toml"));
    assert!(
        matches!(result, Err(CodegenError::MissingSrcDir(_))),
        "should produce MissingSrcDir"
    );
}

// -- Edge cases --

#[test]
fn defaults_version_to_zero_when_missing() {
    let tmp = TempDir::new().unwrap();
    create_crate(tmp.path(), "[package]\nname = \"no_version\"\n", true);

    let meta = CrateMetadata::from_cargo_toml(&tmp.path().join("Cargo.toml")).unwrap();
    assert_eq!(meta.version, "0.0.0");
}
