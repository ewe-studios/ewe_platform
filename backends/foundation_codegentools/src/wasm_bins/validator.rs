use std::path::Path;

use super::error::WasmBinError;

/// WHY: Not all crates are valid targets for WASM binary generation.
/// We need to check the Cargo.toml before scanning to give clear errors.
///
/// WHAT: Validates that a crate's Cargo.toml is compatible with WASM binary
/// generation.
///
/// HOW: Reads `[lib].crate-type` from Cargo.toml. Valid if: cdylib, has
/// `[[bin]]` sections, or has no `[lib]` section. Invalid if `[lib]` exists
/// with non-cdylib crate-type.
///
/// # Errors
///
/// Returns `WasmBinError::CrateNotFound` if the directory doesn't exist.
/// Returns `WasmBinError::NoCargoToml` if no Cargo.toml is found.
/// Returns `WasmBinError::CargoTomlParse` if Cargo.toml can't be parsed.
/// Returns `WasmBinError::InvalidCrateType` if the crate-type is incompatible.
pub fn validate_crate(crate_dir: &Path) -> Result<toml::Value, WasmBinError> {
    if !crate_dir.exists() {
        return Err(WasmBinError::CrateNotFound(crate_dir.to_path_buf()));
    }

    let cargo_toml_path = crate_dir.join("Cargo.toml");
    if !cargo_toml_path.exists() {
        return Err(WasmBinError::NoCargoToml(crate_dir.to_path_buf()));
    }

    let content = std::fs::read_to_string(&cargo_toml_path).map_err(|e| WasmBinError::Io {
        path: cargo_toml_path.clone(),
        source: e,
    })?;

    let parsed: toml::Value =
        toml::from_str(&content).map_err(|e| WasmBinError::CargoTomlParse {
            path: cargo_toml_path,
            source: e,
        })?;

    // Check if crate has [[bin]] sections — always valid
    if parsed.get("bin").is_some() {
        return Ok(parsed);
    }

    // Check [lib] section
    if let Some(lib) = parsed.get("lib") {
        if let Some(crate_type) = lib.get("crate-type") {
            if let Some(types) = crate_type.as_array() {
                let has_cdylib = types
                    .iter()
                    .any(|t| t.as_str().is_some_and(|s| s == "cdylib"));
                if !has_cdylib {
                    let type_str = types
                        .iter()
                        .filter_map(toml::Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(WasmBinError::InvalidCrateType {
                        crate_dir: crate_dir.to_path_buf(),
                        crate_type: type_str,
                    });
                }
            }
        }
        // [lib] exists but no crate-type means default (rlib) — invalid
        else {
            return Err(WasmBinError::InvalidCrateType {
                crate_dir: crate_dir.to_path_buf(),
                crate_type: "rlib (default)".to_string(),
            });
        }
    }

    // No [lib] section — valid (allows adding [[bin]] freely)
    Ok(parsed)
}
