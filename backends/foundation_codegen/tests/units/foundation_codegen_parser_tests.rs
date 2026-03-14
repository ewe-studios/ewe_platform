//! Unit tests for `parse_rust_file` in `foundation_codegen`.
//!
//! Validates successful parsing of valid Rust source and proper
//! error handling for invalid syntax and missing files.

use std::fs;
use std::path::Path;

use foundation_codegen::parser::parse_rust_file;
use tempfile::TempDir;

/// Helper: write a Rust source file and return its path.
fn write_source(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::write(&path, content).unwrap();
    path
}

// -- Valid input: parses correct Rust source --

#[test]
fn parses_valid_rust_file() {
    let tmp = TempDir::new().unwrap();
    let path = write_source(tmp.path(), "lib.rs", "pub struct Foo;\nfn bar() {}\n");

    let ast = parse_rust_file(&path).unwrap();
    assert_eq!(ast.items.len(), 2, "should parse both items");
}

// -- Invalid input: rejects bad syntax --

#[test]
fn error_on_invalid_rust_syntax() {
    let tmp = TempDir::new().unwrap();
    let path = write_source(tmp.path(), "bad.rs", "fn { broken syntax");

    let result = parse_rust_file(&path);
    assert!(result.is_err(), "should fail on invalid Rust syntax");
}

// -- Invalid input: missing file --

#[test]
fn error_on_missing_file() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("nonexistent.rs");

    let result = parse_rust_file(&path);
    assert!(result.is_err(), "should fail when file does not exist");
}
