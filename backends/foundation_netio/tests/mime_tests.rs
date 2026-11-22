//! Tests for `foundation_netio::shared::mime`.

use std::path::PathBuf;

use foundation_netio::shared::mime::{from_extension, from_path, OCTET_STREAM, TYPES};

#[test]
fn known_rows_resolve() {
    assert_eq!(from_extension("mjs"), Some("text/javascript"));
    assert_eq!(from_extension("js"), Some("text/javascript"));
    assert_eq!(from_extension("wasm"), Some("application/wasm"));
    assert_eq!(from_extension("html"), Some("text/html"));
    assert_eq!(from_extension("svg"), Some("image/svg+xml"));
    assert_eq!(from_extension("txt"), Some("text/plain"));
}

#[test]
fn the_table_is_the_vendored_data_file() {
    // The data test checks the file. This check proves the module loaded it.
    assert!(TYPES.len() >= 1_200);
    assert_eq!(TYPES.get("mjs"), Some(&"text/javascript"));
}

#[test]
fn the_fallback_is_the_callers_choice() {
    assert_eq!(OCTET_STREAM, "application/octet-stream");
    assert_eq!(from_extension("nope"), None);
}

#[test]
fn lookup_ignores_case_and_a_leading_dot() {
    assert_eq!(from_extension("MJS"), Some("text/javascript"));
    assert_eq!(from_extension("Mjs"), Some("text/javascript"));
    assert_eq!(from_extension(".mjs"), Some("text/javascript"));
    assert_eq!(from_extension(".MJS"), Some("text/javascript"));
}

#[test]
fn unknown_names_return_none() {
    assert_eq!(from_extension(""), None);
    assert_eq!(from_extension("."), None);
    assert_eq!(from_extension("no-such-extension"), None);
}

#[test]
fn from_path_reads_the_last_extension() {
    assert_eq!(from_path("/a/file.mjs"), Some("text/javascript"));
    assert_eq!(from_path("file.tar.gz"), Some("application/gzip"));
    assert_eq!(from_path(PathBuf::from("/a/page.html")), Some("text/html"));
}

#[test]
fn a_name_without_an_extension_returns_none() {
    assert_eq!(from_path("/a/file"), None);
    assert_eq!(from_path("/a/.bashrc"), None);
    assert_eq!(from_path(""), None);
}

#[test]
fn every_row_resolves_from_its_extension_and_path() {
    for (extension, media_type) in TYPES.entries() {
        assert_eq!(from_extension(extension), Some(*media_type));
        let path = format!("/a/file.{extension}");
        assert_eq!(from_path(path), Some(*media_type));
    }
}
