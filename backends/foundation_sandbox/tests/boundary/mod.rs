//! The source scans of invariants I10 and I12 (feature 06).
//!
//! I10: a boundary holds no sandbox logic. I12: a blocking drive appears only
//! at a boundary. The scan reads the crate source, so it follows every later
//! change in the same files.

use std::path::{Path, PathBuf};

/// The directory of the crate source.
fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every `.rs` file below `path`.
fn rust_files(path: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            found.push(path);
        }
    }
}

/// A path relative to `src`, with `/` separators.
fn relative(path: &Path) -> String {
    path.strip_prefix(source_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The files that feature 06 names as boundaries.
fn is_boundary(path: &str) -> bool {
    path.starts_with("cli/")
        || path.starts_with("providers/")
        || path.starts_with("bin/")
        || path.ends_with("control/service.rs")
        || path.ends_with("native/connect.rs")
        || path.ends_with("native/serve.rs")
}

#[test]
fn blocking_only_at_boundaries() {
    let mut files = Vec::new();
    rust_files(&source_root(), &mut files);
    let mut offenders = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("the source file");
        if !text.contains("block_on_future(") {
            continue;
        }
        let path = relative(file);
        // The `Provider` methods and the two programs block. The CLI module is
        // the body of the `agent-sandbox` main function, so it is a boundary
        // too; section 4.2 I12 names "main functions", and `cli::sandbox::run`
        // is that function.
        if !(path.starts_with("providers/") || path.starts_with("bin/") || path.starts_with("cli/"))
        {
            offenders.push(path);
        }
    }
    assert!(
        offenders.is_empty(),
        "block_on_future appears outside the boundaries: {offenders:?}"
    );
}

#[test]
fn boundaries_have_no_logic() {
    let mut files = Vec::new();
    rust_files(&source_root(), &mut files);
    let mut offenders = Vec::new();
    for file in &files {
        let path = relative(file);
        if !is_boundary(&path) {
            continue;
        }
        let text = std::fs::read_to_string(file).expect("the source file");
        for needle in [".validate(", "boot_timeout", "Duration::from_"] {
            if text.contains(needle) {
                offenders.push(format!("{path}: {needle}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a boundary holds sandbox logic: {offenders:?}"
    );
}
