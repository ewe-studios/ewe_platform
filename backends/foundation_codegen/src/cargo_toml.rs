use std::path::Path;

use serde::Deserialize;

use crate::error::{CodegenError, Result};
use crate::types::CrateMetadata;

#[derive(Deserialize)]
struct CargoToml {
    package: Option<PackageSection>,
    lib: Option<LibSection>,
}

#[derive(Deserialize)]
struct PackageSection {
    name: Option<String>,
    version: Option<String>,
}

#[derive(Deserialize)]
struct LibSection {
    path: Option<String>,
}

impl CrateMetadata {
    /// WHY: The scanner needs crate-level context (name, source root) before
    /// it can resolve module paths for discovered items.
    ///
    /// WHAT: Reads and parses a `Cargo.toml` file into `CrateMetadata`.
    ///
    /// HOW: Deserializes `[package]` and optional `[lib]` sections with `toml`,
    /// then probes the filesystem for `src/lib.rs` or `src/main.rs`.
    ///
    /// # Errors
    ///
    /// Returns `CodegenError` if the file cannot be read, parsed, or is missing
    /// required fields (`[package]`, `name`), or if the `src/` directory is absent.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn from_cargo_toml(cargo_toml_path: &Path) -> Result<Self> {
        let cargo_toml_path = cargo_toml_path
            .canonicalize()
            .map_err(|e| CodegenError::Io {
                path: cargo_toml_path.to_path_buf(),
                source: e,
            })?;

        if !cargo_toml_path.exists() {
            return Err(CodegenError::MissingCargoToml(cargo_toml_path));
        }

        let contents = std::fs::read_to_string(&cargo_toml_path).map_err(|e| CodegenError::Io {
            path: cargo_toml_path.clone(),
            source: e,
        })?;

        let parsed: CargoToml =
            toml::from_str(&contents).map_err(|e| CodegenError::CargoTomlError {
                path: cargo_toml_path.clone(),
                source: e,
            })?;

        let package = parsed
            .package
            .ok_or_else(|| CodegenError::MissingPackageSection(cargo_toml_path.clone()))?;

        let name = package
            .name
            .ok_or_else(|| CodegenError::MissingPackageName(cargo_toml_path.clone()))?;

        let version = package.version.unwrap_or_else(|| "0.0.0".to_string());

        // A canonicalized path always has a parent directory
        let root_dir = cargo_toml_path
            .parent()
            .unwrap_or(&cargo_toml_path)
            .to_path_buf();

        let src_dir = root_dir.join("src");
        if !src_dir.exists() {
            return Err(CodegenError::MissingSrcDir(root_dir));
        }

        // Determine entry point: custom [lib] path, or default lib.rs/main.rs
        let (entry_point, is_lib) = if let Some(lib_section) = &parsed.lib {
            if let Some(custom_path) = &lib_section.path {
                (root_dir.join(custom_path), true)
            } else {
                default_entry_point(&src_dir)?
            }
        } else {
            default_entry_point(&src_dir)?
        };

        Ok(CrateMetadata {
            name,
            version,
            root_dir,
            cargo_toml_path,
            src_dir,
            entry_point,
            is_lib,
        })
    }
}

/// Determine the default entry point from the src directory.
fn default_entry_point(src_dir: &Path) -> Result<(std::path::PathBuf, bool)> {
    let lib_rs = src_dir.join("lib.rs");
    let main_rs = src_dir.join("main.rs");

    if lib_rs.exists() {
        Ok((lib_rs, true))
    } else if main_rs.exists() {
        Ok((main_rs, false))
    } else {
        Err(CodegenError::MissingSrcDir(
            src_dir.parent().unwrap_or(src_dir).to_path_buf(),
        ))
    }
}
