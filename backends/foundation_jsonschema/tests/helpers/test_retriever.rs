//! Test retriever — loads remote schemas for the official test suite.
//!
//! WHY: The official JSON Schema Test Suite's `$ref` tests reference
//! schemas like "http://localhost:1234/integer.json". Rather than running
//! an HTTP server, we load these from the vendored `remotes/` directory
//! into an `InMemoryFetcher`.
//!
//! WHAT: `test_retriever()` returns an `InMemoryFetcher` pre-loaded with
//! all remote schemas from `tests/suite/remotes/`.
//!
//! HOW: Walks the `remotes/` directory recursively, mapping each file to
//! the expected `http://localhost:1234/...` URI.

use std::fs;
use std::path::Path;

use foundation_jsonschema::InMemoryFetcher;
use serde_json::Value;

/// Create an `InMemoryFetcher` pre-loaded with all remote schemas
/// from the official test suite's `remotes/` directory.
///
/// The URIs are mapped to `http://localhost:1234/<path>` as expected
/// by the test suite.
pub fn test_retriever() -> InMemoryFetcher {
    let mut fetcher = InMemoryFetcher::builtin();

    let remotes_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("suite")
        .join("remotes");

    load_remote_dir(&remotes_dir, "http://localhost:1234", &mut fetcher);

    fetcher
}

/// Recursively load all JSON files from a directory into the fetcher.
fn load_remote_dir(dir: &Path, base_uri: &str, fetcher: &mut InMemoryFetcher) {
    if !dir.is_dir() {
        return;
    }

    for entry in fs::read_dir(dir).expect("failed to read remotes dir") {
        let entry = entry.expect("failed to read entry");
        let path = entry.path();

        if path.is_dir() {
            // Subdirectory: use directory name as URI segment
            let subdir_name = path.file_name().unwrap().to_str().unwrap();
            load_remote_dir(&path, &format!("{base_uri}/{subdir_name}"), fetcher);
        } else if path.extension().is_some_and(|e| e == "json") {
            let filename = path.file_name().unwrap().to_str().unwrap();
            let uri = format!("{base_uri}/{filename}");
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
            let schema: Value = serde_json::from_str(&content)
                .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
            fetcher.insert(uri, schema);
        }
    }
}
