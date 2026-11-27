//! The source scan of invariant I7 (spec 77 feature 08, section 7).
//!
//! The Cloudflare Worker crate has no sandbox logic. Its state checks, its
//! boot deadlines and its retries stay in `foundation_sandbox`.

use std::fs;
use std::path::{Path, PathBuf};

/// Words that belong to the control plane, never to the Worker.
const FORBIDDEN: &[&str] = &["SandboxState", "boot_timeout", "retry"];

/// Every Rust source file under `directory`.
fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(directory).expect("the Worker source directory reads");
    for entry in entries {
        let entry = entry.expect("a Worker source entry reads");
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, files);
        } else if path.extension().and_then(std::ffi::OsStr::to_str) == Some("rs") {
            files.push(path);
        }
    }
}

#[test]
fn worker_has_no_logic() {
    let directory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../infrastructure/sandbox_worker/src");
    assert!(
        directory.is_dir(),
        "the sandbox Worker source is absent at {}",
        directory.display()
    );
    let mut files = Vec::new();
    rust_files(&directory, &mut files);
    assert!(!files.is_empty(), "the sandbox Worker source is empty");

    for word in FORBIDDEN {
        for file in &files {
            let text = fs::read_to_string(file).expect("a Worker source file reads");
            assert!(
                !text.contains(word),
                "{} holds {word}; the Worker keeps no sandbox logic",
                file.display()
            );
        }
    }
}
