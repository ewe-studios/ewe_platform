use std::path::Path;

use super::error::WasmBinError;
use super::WasmBinPlan;

/// WHY: The generate command needs to write binary source files and update
/// Cargo.toml with `[[bin]]` sections.
///
/// WHAT: Writes all generated files to disk and updates Cargo.toml.
///
/// HOW: Creates `bin/{name}/main.rs` files for each entrypoint, then uses
/// `toml_edit` to append `[[bin]]` sections while preserving formatting.
///
/// # Errors
///
/// Returns `WasmBinError::Io` if file creation or Cargo.toml modification fails.
///
/// # Panics
///
/// Never panics.
pub fn execute_plan(plan: &WasmBinPlan) -> Result<(), WasmBinError> {
    // Create binary source files
    for file in &plan.generated_files {
        let full_path = plan.crate_dir.join(&file.path);

        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| WasmBinError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        std::fs::write(&full_path, &file.content).map_err(|e| WasmBinError::Io {
            path: full_path,
            source: e,
        })?;
    }

    // Update Cargo.toml with [[bin]] sections
    update_cargo_toml(&plan.crate_dir, plan)?;

    Ok(())
}

/// WHY: Cargo.toml must have `[[bin]]` sections for each generated binary
/// so `cargo build` can discover and compile them.
///
/// WHAT: Appends `[[bin]]` entries to Cargo.toml using `toml_edit` to
/// preserve existing formatting.
///
/// HOW: Reads Cargo.toml as a `toml_edit::DocumentMut`, checks for existing
/// `[[bin]]` entries to avoid duplicates, then appends new ones.
///
/// # Errors
///
/// Returns `WasmBinError::Io` on file read/write errors.
///
/// # Panics
///
/// Never panics.
fn update_cargo_toml(crate_dir: &Path, plan: &WasmBinPlan) -> Result<(), WasmBinError> {
    let cargo_path = crate_dir.join("Cargo.toml");

    let content = std::fs::read_to_string(&cargo_path).map_err(|e| WasmBinError::Io {
        path: cargo_path.clone(),
        source: e,
    })?;

    let mut doc: toml_edit::DocumentMut =
        content
            .parse()
            .map_err(|e: toml_edit::TomlError| WasmBinError::Io {
                path: cargo_path.clone(),
                source: std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
            })?;

    // Collect existing bin names to avoid duplicates
    let existing_names: Vec<String> = collect_existing_bin_names(&doc);

    for bin in &plan.bin_sections {
        if existing_names.contains(&bin.name) {
            continue;
        }

        if doc.get("bin").is_none() {
            doc.insert(
                "bin",
                toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new()),
            );
        }

        let bins = doc["bin"]
            .as_array_of_tables_mut()
            .expect("bin should be array of tables");

        let mut table = toml_edit::Table::new();
        table["name"] = toml_edit::value(&bin.name);
        table["path"] = toml_edit::value(&bin.path);
        bins.push(table);
    }

    std::fs::write(&cargo_path, doc.to_string()).map_err(|e| WasmBinError::Io {
        path: cargo_path,
        source: e,
    })?;

    Ok(())
}

fn collect_existing_bin_names(doc: &toml_edit::DocumentMut) -> Vec<String> {
    let Some(item) = doc.get("bin") else {
        return Vec::new();
    };
    let Some(bins) = item.as_array_of_tables() else {
        return Vec::new();
    };
    bins.iter()
        .filter_map(|t| t.get("name")?.as_str().map(String::from))
        .collect()
}
