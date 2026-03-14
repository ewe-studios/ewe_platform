//! Unit tests for the `CodegenError` type in `foundation_codegen`.
//!
//! Validates Display messages include relevant context and that
//! `Error::source()` delegates correctly for wrapped error variants.

use std::path::PathBuf;

use foundation_codegen::CodegenError;

// -- Valid output: Display messages contain the path context --

#[test]
fn display_io_error_includes_path_and_cause() {
    let err = CodegenError::Io {
        path: PathBuf::from("/tmp/foo.rs"),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/tmp/foo.rs"), "should contain the file path");
    assert!(msg.contains("not found"), "should contain the IO cause");
}

#[test]
fn display_parse_error_includes_path_and_message() {
    let err = CodegenError::ParseError {
        path: PathBuf::from("/tmp/bad.rs"),
        message: "unexpected token".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("/tmp/bad.rs"));
    assert!(msg.contains("unexpected token"));
}

#[test]
fn display_missing_cargo_toml_includes_path() {
    let err = CodegenError::MissingCargoToml(PathBuf::from("/fake/Cargo.toml"));
    assert!(err.to_string().contains("/fake/Cargo.toml"));
}

#[test]
fn display_missing_package_section_mentions_package() {
    let err = CodegenError::MissingPackageSection(PathBuf::from("/fake/Cargo.toml"));
    assert!(err.to_string().contains("[package]"));
}

#[test]
fn display_missing_package_name_mentions_field() {
    let err = CodegenError::MissingPackageName(PathBuf::from("/fake/Cargo.toml"));
    assert!(err.to_string().contains("package.name"));
}

#[test]
fn display_missing_src_dir_mentions_source_directory() {
    let err = CodegenError::MissingSrcDir(PathBuf::from("/fake/crate"));
    assert!(err.to_string().contains("source directory"));
}

// -- Error::source() delegation --

#[test]
fn source_delegates_for_io_variant() {
    let err = CodegenError::Io {
        path: PathBuf::from("/tmp/foo.rs"),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
    };
    assert!(
        std::error::Error::source(&err).is_some(),
        "Io variant should delegate to inner io::Error"
    );
}

#[test]
fn source_returns_none_for_simple_variants() {
    let err = CodegenError::MissingCargoToml(PathBuf::from("/fake"));
    assert!(
        std::error::Error::source(&err).is_none(),
        "simple variants have no inner source"
    );
}
