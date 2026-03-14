//! Unit tests for file walking in `foundation_codegen`.
//!
//! Validates that `find_rust_files` discovers `.rs` files recursively,
//! skips hidden dirs and `target/`, returns sorted absolute paths,
//! and handles edge cases (empty dirs, non-rs files).

use std::fs;
use std::path::Path;

use foundation_codegen::file_walker::find_rust_files;
use tempfile::TempDir;

/// Helper: create a file at a nested path, creating parent dirs as needed.
fn touch(base: &Path, relative: &str) {
    let path = base.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, "// placeholder").unwrap();
}

// -- Valid input: finds .rs files recursively --

#[test]
fn finds_rs_files_in_flat_directory() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "src/lib.rs");
    touch(tmp.path(), "src/main.rs");

    let files = find_rust_files(&tmp.path().join("src")).unwrap();
    assert_eq!(files.len(), 2);
    assert!(files.iter().all(|p| p.extension().is_some_and(|e| e == "rs")));
}

#[test]
fn finds_rs_files_in_nested_directories() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "src/lib.rs");
    touch(tmp.path(), "src/handlers/mod.rs");
    touch(tmp.path(), "src/handlers/auth.rs");
    touch(tmp.path(), "src/models/user.rs");

    let files = find_rust_files(&tmp.path().join("src")).unwrap();
    assert_eq!(files.len(), 4);
}

// -- Invalid/filtered input: skips hidden dirs and target/ --

#[test]
fn skips_hidden_directories() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "src/lib.rs");
    touch(tmp.path(), "src/.hidden/secret.rs");

    let files = find_rust_files(&tmp.path().join("src")).unwrap();
    assert_eq!(files.len(), 1, "should skip files in hidden directories");
}

#[test]
fn skips_target_directory() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "src/lib.rs");
    touch(tmp.path(), "src/target/debug/build.rs");

    let files = find_rust_files(&tmp.path().join("src")).unwrap();
    assert_eq!(files.len(), 1, "should skip files in target/");
}

#[test]
fn ignores_non_rs_files() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "src/lib.rs");
    touch(tmp.path(), "src/readme.md");
    touch(tmp.path(), "src/data.json");

    let files = find_rust_files(&tmp.path().join("src")).unwrap();
    assert_eq!(files.len(), 1, "should only return .rs files");
}

// -- Edge case: empty directory --

#[test]
fn returns_empty_vec_for_empty_directory() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();

    let files = find_rust_files(&src).unwrap();
    assert!(files.is_empty());
}

// -- Output ordering: results are sorted --

#[test]
fn returns_sorted_paths() {
    let tmp = TempDir::new().unwrap();
    touch(tmp.path(), "src/z_module.rs");
    touch(tmp.path(), "src/a_module.rs");
    touch(tmp.path(), "src/m_module.rs");

    let files = find_rust_files(&tmp.path().join("src")).unwrap();
    let sorted: Vec<_> = {
        let mut v = files.clone();
        v.sort();
        v
    };
    assert_eq!(files, sorted, "results should be sorted");
}
