use std::path::Path;

use crate::error::{CodegenError, Result};
use crate::types::CrateMetadata;

/// WHY: The scanner discovers items in files, but consumers need the full
/// Rust module path (e.g., `my_crate::handlers::auth`) to generate imports
/// and qualified references.
///
/// WHAT: Converts filesystem paths to Rust module paths, handling special
/// cases like `lib.rs`, `main.rs`, and `mod.rs`.
///
/// HOW: Uses `CrateMetadata` for crate name and `src_dir`, then strips the
/// `src_dir` prefix, handles special filenames, and joins segments with `::`.
///
/// # Limitations
///
/// This resolver does NOT handle:
/// - `#[path = "..."]` attributes on module declarations
/// - `cfg`-gated modules (all files are scanned regardless)
/// - Re-exports via `pub use`
pub struct ModulePathResolver {
    crate_metadata: CrateMetadata,
}

impl ModulePathResolver {
    /// WHY: Each resolver needs crate context (name, `src_dir`) to compute
    /// module paths correctly.
    ///
    /// WHAT: Creates a new resolver bound to a specific crate.
    ///
    /// HOW: Stores the `CrateMetadata` for use in resolution methods.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn new(crate_metadata: CrateMetadata) -> Self {
        Self { crate_metadata }
    }

    /// WHY: Each `.rs` file maps to a Rust module path based on its location
    /// relative to the crate root and its filename.
    ///
    /// WHAT: Resolves a file path to its module path (e.g., `my_crate::handlers`).
    ///
    /// HOW: Strips `src_dir` prefix, removes `.rs` extension, handles special
    /// cases (`lib.rs`/`main.rs` → crate root, `mod.rs` → parent dir), then
    /// joins path components with `::`.
    ///
    /// # Errors
    ///
    /// Returns `CodegenError::Io` if the path cannot be made relative to `src_dir`.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn resolve_file_module_path(&self, file_path: &Path) -> Result<String> {
        // Step 1: Make file_path relative to src_dir
        let relative = file_path
            .strip_prefix(&self.crate_metadata.src_dir)
            .map_err(|_| CodegenError::Io {
                path: file_path.to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "Path {} is not relative to {}",
                        file_path.display(),
                        self.crate_metadata.src_dir.display()
                    ),
                ),
            })?;

        // Step 2: Strip .rs extension
        let without_extension = relative.with_extension("");

        // Step 3: Collect path components
        let mut segments: Vec<String> = Vec::new();

        for component in without_extension.components() {
            if let std::path::Component::Normal(os_str) = component {
                if let Some(name) = os_str.to_str() {
                    segments.push(name.to_string());
                }
            }
        }

        // Step 4: Handle special cases
        if let Some(last) = segments.last() {
            match last.as_str() {
                // lib.rs / main.rs → crate root (just crate name)
                "lib" | "main" => {
                    return Ok(self.crate_metadata.name.clone());
                }
                // mod.rs → use parent directory, drop "mod"
                "mod" => {
                    segments.pop();
                }
                _ => {}
            }
        }

        // Step 5: Build module path with crate name prefix
        if segments.is_empty() {
            Ok(self.crate_metadata.name.clone())
        } else {
            let mut full_path = vec![self.crate_metadata.name.clone()];
            full_path.extend(segments);
            Ok(full_path.join("::"))
        }
    }

    /// WHY: Items may be nested inside inline `mod` blocks within a file,
    /// which adds to the module path.
    ///
    /// WHAT: Combines the file-based module path with inline module nesting.
    ///
    /// HOW: Resolves the file path, then appends inline modules joined by `::`.
    ///
    /// # Errors
    ///
    /// Returns `CodegenError::Io` if the underlying file path resolution fails.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn resolve_item_module_path(
        &self,
        file_path: &Path,
        inline_module_path: &[String],
    ) -> Result<String> {
        let file_module = self.resolve_file_module_path(file_path)?;
        if inline_module_path.is_empty() {
            Ok(file_module)
        } else {
            Ok(format!(
                "{}::{}",
                file_module,
                inline_module_path.join("::")
            ))
        }
    }

    /// WHY: Consumers need the complete path to an item for generating imports
    /// and type references.
    ///
    /// WHAT: Returns the fully qualified path including the item name.
    ///
    /// HOW: Resolves the item module path and appends the item name.
    ///
    /// # Errors
    ///
    /// Returns `CodegenError::Io` if the underlying file path resolution fails.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn resolve_qualified_path(
        &self,
        file_path: &Path,
        inline_module_path: &[String],
        item_name: &str,
    ) -> Result<String> {
        let module_path = self.resolve_item_module_path(file_path, inline_module_path)?;
        Ok(format!("{module_path}::{item_name}"))
    }
}
