use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::error::Result;

/// WHY: The scanner needs to discover all Rust source files in a crate
/// before parsing them for macro annotations.
///
/// WHAT: Recursively walks a directory and returns all `.rs` file paths.
///
/// HOW: Uses `walkdir` for recursive traversal, skips hidden directories
/// and `target/`, canonicalizes paths, and sorts for deterministic output.
///
/// # Errors
///
/// Returns `CodegenError::Io` if the directory cannot be read.
///
/// # Panics
///
/// Never panics.
pub fn find_rust_files(src_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(src_dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "target"
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
    {
        files.push(entry.into_path());
    }

    files.sort();
    Ok(files)
}
