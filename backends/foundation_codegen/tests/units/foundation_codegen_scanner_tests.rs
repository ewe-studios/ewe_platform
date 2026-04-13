//! Unit tests for `SourceScanner` in `foundation_codegen`.
//!
//! Validates single-file scanning, directory scanning with recursive
//! discovery, error tolerance for unparseable files, and correct
//! attribute extraction across multiple files.

use std::fs;
use std::path::Path;

use foundation_codegen::scanner::SourceScanner;
use foundation_codegen::ItemKind;
use tempfile::TempDir;

/// Helper: write a Rust source file at a nested path.
fn write_source(base: &Path, relative: &str, content: &str) {
    let path = base.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

// -- Valid input: scan_file finds annotated items --

#[test]
fn scan_file_finds_annotated_struct() {
    let tmp = TempDir::new().unwrap();
    write_source(
        tmp.path(),
        "lib.rs",
        r"
        #[module]
        pub struct Handler;
        ",
    );

    let scanner = SourceScanner::new("module");
    let items = scanner.scan_file(&tmp.path().join("lib.rs")).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item_name, "Handler");
    assert_eq!(items[0].item_kind, ItemKind::Struct);
}

// -- Valid input: scan_directory finds items across files --

#[test]
fn scan_directory_finds_items_across_multiple_files() {
    let tmp = TempDir::new().unwrap();
    write_source(
        tmp.path(),
        "src/lib.rs",
        r"
        #[component]
        pub struct App;
        ",
    );
    write_source(
        tmp.path(),
        "src/handlers/auth.rs",
        r"
        #[component]
        pub struct AuthHandler;

        #[component]
        pub fn login() {}
        ",
    );
    write_source(
        tmp.path(),
        "src/models/user.rs",
        r"
        pub struct User;  // not annotated
        ",
    );

    let scanner = SourceScanner::new("component");
    let items = scanner.scan_directory(&tmp.path().join("src")).unwrap();
    assert_eq!(
        items.len(),
        3,
        "should find 3 annotated items across 2 files"
    );
}

// -- Error tolerance: unparseable files are skipped --

#[test]
fn scan_directory_skips_unparseable_files() {
    let tmp = TempDir::new().unwrap();
    write_source(
        tmp.path(),
        "src/good.rs",
        r"
        #[module]
        pub struct Good;
        ",
    );
    write_source(tmp.path(), "src/bad.rs", "fn { broken syntax");

    let scanner = SourceScanner::new("module");
    let items = scanner.scan_directory(&tmp.path().join("src")).unwrap();
    assert_eq!(
        items.len(),
        1,
        "should still find items from parseable files"
    );
    assert_eq!(items[0].item_name, "Good");
}

// -- Edge case: empty directory --

#[test]
fn scan_directory_returns_empty_for_no_annotated_items() {
    let tmp = TempDir::new().unwrap();
    write_source(tmp.path(), "src/lib.rs", "pub struct Plain;");

    let scanner = SourceScanner::new("module");
    let items = scanner.scan_directory(&tmp.path().join("src")).unwrap();
    assert!(items.is_empty());
}

// -- Edge case: scan_file error on missing file --

#[test]
fn scan_file_errors_on_missing_file() {
    let scanner = SourceScanner::new("module");
    let result = scanner.scan_file(Path::new("/nonexistent/file.rs"));
    assert!(result.is_err());
}
