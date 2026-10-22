//! Stage the project templates into `$OUT_DIR/templates`, where
//! `#[derive(EmbedDirectoryAs)]` in `src/generate` embeds them into the binary.
//!
//! The templates live inside this crate (`templates/`) so the published package is
//! self-contained. Each template's manifest is stored as `Cargo.toml.template`: a
//! directory holding a `Cargo.toml` is a separate package to Cargo, which would leave
//! it out of this crate's package (and treat it as a crate of its own). The staged
//! copy restores the real name, so the embedded templates are unchanged.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

/// Source directory of the templates, relative to this crate's manifest.
const TEMPLATES_SOURCE: &str = "templates";

/// Suffix of a template file stored under a name Cargo must not see.
const STORED_SUFFIX: &str = ".template";

/// File names that must be stored with [`STORED_SUFFIX`] and renamed when staged.
const RENAMED_ON_STAGE: &[&str] = &["Cargo.toml"];

/// The staged name for a stored file name: `Cargo.toml.template` becomes
/// `Cargo.toml`; every other name is kept.
fn staged_name(stored: &str) -> &str {
    match stored.strip_suffix(STORED_SUFFIX) {
        Some(original) if RENAMED_ON_STAGE.contains(&original) => original,
        _ => stored,
    }
}

/// Recursively copy `from` into `to`, restoring stored file names.
fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("non UTF-8 template path: {}", path.display()),
            )
        })?;

        if entry.file_type()?.is_dir() {
            copy_dir(&path, &to.join(name))?;
        } else {
            fs::copy(&path, to.join(staged_name(name)))?;
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let package_directory = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").map_err(|e| io::Error::other(format!("CARGO_MANIFEST_DIR: {e}")))?,
    );
    let out_directory =
        PathBuf::from(env::var("OUT_DIR").map_err(|e| io::Error::other(format!("OUT_DIR: {e}")))?);

    let source_directory = package_directory.join(TEMPLATES_SOURCE);
    let output_directory = out_directory.join("templates");
    println!("cargo:rerun-if-changed={TEMPLATES_SOURCE}");

    // Start from a clean copy so removed templates do not linger.
    if output_directory.exists() {
        fs::remove_dir_all(&output_directory)?;
    }
    copy_dir(&source_directory, &output_directory)
}
