use std::path::Path;

use system_operations::wasm_bins::validator::validate_crate;

#[test]
fn validate_crate_accepts_cdylib_crate() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/wasm_crate");
    let result = validate_crate(&fixture_dir);
    assert!(result.is_ok(), "cdylib crate should be valid: {result:?}");
}

#[test]
fn validate_crate_rejects_rlib_crate() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rlib_crate");
    let result = validate_crate(&fixture_dir);
    assert!(result.is_err(), "rlib crate should be rejected");
}

#[test]
fn validate_crate_rejects_nonexistent_directory() {
    let result = validate_crate(Path::new("/nonexistent/path"));
    assert!(result.is_err(), "nonexistent directory should be rejected");
}

#[test]
fn validate_crate_rejects_directory_without_cargo_toml() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let result = validate_crate(&fixture_dir);
    assert!(
        result.is_err(),
        "directory without Cargo.toml should be rejected"
    );
}
