use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::module_path::ModulePathResolver;
use crate::registry::ScanRegistry;
use crate::scanner::SourceScanner;
use crate::types::{AttributeValue, CrateMetadata, DerivedTarget, ItemKind};

/// WHY: Consumers need a single, high-level API to scan crates and obtain
/// a complete registry of all macro-annotated items with resolved module paths.
///
/// WHAT: Top-level scanner that ties together source scanning and module
/// path resolution into `scan_crate()`, `scan_crates()`, and `scan_workspace()` methods.
///
/// HOW: Uses `SourceScanner` to find items, `ModulePathResolver` to compute
/// paths, and builds `DerivedTarget` entries for the `ScanRegistry`.
pub struct CrateScanner {
    target_attr: String,
}

impl CrateScanner {
    /// WHY: The scanner is generic over any attribute name so it is not
    /// hardcoded to a single macro.
    ///
    /// WHAT: Creates a new scanner that searches for the given attribute.
    ///
    /// HOW: Stores the attribute name for use in the underlying `SourceScanner`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use foundation_codegen::CrateScanner;
    /// let scanner = CrateScanner::new("module");
    /// // Will find all items annotated with #[module(...)]
    /// ```
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn new(target_attr: &str) -> Self {
        Self {
            target_attr: target_attr.to_string(),
        }
    }

    /// WHY: The primary use case is scanning a single crate to discover all
    /// macro-annotated items with their full metadata.
    ///
    /// WHAT: Scans a crate and returns a registry mapping item names to `DerivedTarget`.
    ///
    /// HOW: Parses `Cargo.toml` for metadata, scans source files with `SourceScanner`,
    /// resolves module paths with `ModulePathResolver`, and builds `DerivedTarget` entries.
    ///
    /// # Arguments
    ///
    /// * `crate_path` - Path to the crate root directory (containing `Cargo.toml`)
    ///
    /// # Returns
    ///
    /// A `ScanRegistry` (`HashMap<String, DerivedTarget>`) mapping item names to metadata.
    ///
    /// # Errors
    ///
    /// Returns `CodegenError::Io` if the Cargo.toml or source files cannot be read.
    /// Returns `CodegenError::CargoTomlError` if the Cargo.toml is invalid.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use foundation_codegen::{CrateScanner, Result};
    /// # fn main() -> Result<()> {
    /// let scanner = CrateScanner::new("module");
    /// let registry = scanner.scan_crate(std::path::Path::new("path/to/my_crate"))?;
    ///
    /// for (name, target) in &registry {
    ///     println!("{}: {} in {}", name, target.item_kind, target.module_path);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn scan_crate(&self, crate_path: &Path) -> Result<ScanRegistry> {
        // 1. Parse CrateMetadata from Cargo.toml
        let cargo_toml = crate_path.join("Cargo.toml");
        let metadata = CrateMetadata::from_cargo_toml(&cargo_toml)?;

        // 2. Create scanner and path resolver
        let scanner = SourceScanner::new(&self.target_attr);
        let resolver = ModulePathResolver::new(metadata.clone());

        // 3. Scan all source files
        let found_items = scanner.scan_directory(&metadata.src_dir)?;

        // 4. Enrich FoundItems into DerivedTargets
        let mut registry = ScanRegistry::new();
        for item in found_items {
            let module_path = resolver
                .resolve_item_module_path(&item.location.file_path, &item.inline_module_path)?;
            let qualified_path = format!("{}::{}", module_path, item.item_name);

            let target = DerivedTarget {
                macro_name: item.macro_name,
                attributes: item.attributes,
                item_name: item.item_name.clone(),
                item_kind: item.item_kind,
                location: item.location,
                module_path,
                qualified_path,
                crate_name: metadata.name.clone(),
                crate_root: metadata.root_dir.clone(),
                cargo_toml_path: metadata.cargo_toml_path.clone(),
            };

            registry.insert(item.item_name, target);
        }

        Ok(registry)
    }

    /// WHY: Workspaces contain multiple crates that may all have annotated items;
    /// consumers need to scan them all and merge results.
    ///
    /// WHAT: Scans multiple crates and merges registries using qualified paths as keys.
    ///
    /// HOW: Calls `scan_crate()` for each path, then merges using `qualified_path`
    /// to avoid cross-crate name collisions.
    ///
    /// # Arguments
    ///
    /// * `crate_paths` - Slice of paths to crate root directories
    ///
    /// # Returns
    ///
    /// A merged `ScanRegistry` with all items keyed by qualified path.
    ///
    /// # Errors
    ///
    /// Returns error if any crate scan fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use foundation_codegen::{CrateScanner, Result};
    /// # fn main() -> Result<()> {
    /// let scanner = CrateScanner::new("module");
    /// let registry = scanner.scan_crates(&[
    ///     std::path::Path::new("path/to/crate_a"),
    ///     std::path::Path::new("path/to/crate_b"),
    /// ])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn scan_crates(&self, crate_paths: &[&Path]) -> Result<ScanRegistry> {
        let mut merged = ScanRegistry::new();
        for path in crate_paths {
            let registry = self.scan_crate(path)?;
            for (_, target) in registry {
                // Use qualified_path as key to avoid name collisions
                merged.insert(target.qualified_path.clone(), target);
            }
        }
        Ok(merged)
    }

    /// WHY: Workspaces often use glob patterns in `workspace.members` (e.g., `backends/*`);
    /// consumers need to scan all matching crates.
    ///
    /// WHAT: Scans all crates in a Cargo workspace by reading workspace members.
    ///
    /// HOW: Parses root Cargo.toml, extracts `workspace.members`, expands glob patterns,
    /// and calls `scan_crates()` on each member.
    ///
    /// # Arguments
    ///
    /// * `workspace_root` - Path to workspace root (containing root Cargo.toml)
    ///
    /// # Returns
    ///
    /// A merged `ScanRegistry` with all items from all workspace members.
    ///
    /// # Errors
    ///
    /// Returns `CodegenError::Io` if the Cargo.toml cannot be read.
    /// Returns `CodegenError::CargoTomlError` if parsing fails.
    ///
    /// # Limitations
    ///
    /// Glob patterns in workspace members are NOT expanded — only literal paths are scanned.
    pub fn scan_workspace(&self, workspace_root: &Path) -> Result<ScanRegistry> {
        let cargo_toml_path = workspace_root.join("Cargo.toml");
        let cargo_toml_content = std::fs::read_to_string(&cargo_toml_path).map_err(|e| {
            crate::error::CodegenError::Io {
                path: cargo_toml_path.clone(),
                source: e,
            }
        })?;

        let toml_value: toml::Value = toml::from_str(&cargo_toml_content).map_err(|e| {
            crate::error::CodegenError::CargoTomlError {
                path: cargo_toml_path.clone(),
                source: e,
            }
        })?;

        // Extract workspace.members
        let members = toml_value
            .get("workspace")
            .and_then(|w| w.get("members"))
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Collect crate paths (glob patterns are NOT expanded — documented limitation)
        let mut crate_paths = Vec::new();
        for member in &members {
            if member.contains('*') {
                // Skip glob patterns with a warning
                eprintln!("Warning: glob pattern '{member}' in workspace members is not expanded");
                continue;
            }
            let path = workspace_root.join(member);
            if path.join("Cargo.toml").exists() {
                crate_paths.push(path);
            }
        }

        // Scan each crate
        let path_refs: Vec<&Path> = crate_paths.iter().map(PathBuf::as_path).collect();
        self.scan_crates(&path_refs)
    }
}

/// WHY: Consumers need convenient helpers to filter and group scan results
/// for code generation (e.g., "all items in module X", "only structs").
///
/// WHAT: Extension trait providing common registry query operations.
///
/// HOW: Implemented for `ScanRegistry` (`HashMap<String, DerivedTarget>`).
pub trait RegistryExt {
    /// Group targets by the value of a specific attribute key.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use foundation_codegen::{CrateScanner, RegistryExt};
    /// # fn main() -> foundation_codegen::Result<()> {
    /// # let scanner = CrateScanner::new("module");
    /// # let registry = scanner.scan_crate(std::path::Path::new("path/to/my_crate"))?;
    /// let groups = registry.group_by_attribute("module");
    /// // Returns: {"auth": [AuthHandler, AuthMiddleware], "api": [ApiRouter]}
    /// # Ok(())
    /// # }
    /// ```
    fn group_by_attribute(
        &self,
        attr_key: &str,
    ) -> std::collections::HashMap<String, Vec<&DerivedTarget>>;

    /// Filter targets by item kind.
    fn filter_by_kind(&self, kind: &ItemKind) -> Vec<&DerivedTarget>;

    /// Filter targets by a specific attribute value.
    fn filter_by_attribute(
        &self,
        attr_key: &str,
        attr_value: &AttributeValue,
    ) -> Vec<&DerivedTarget>;

    /// Get all unique values for a specific attribute key.
    fn unique_attribute_values(&self, attr_key: &str) -> Vec<&AttributeValue>;

    /// Filter targets by crate name.
    fn filter_by_crate(&self, crate_name: &str) -> Vec<&DerivedTarget>;

    /// Get all targets sorted by module path.
    fn sorted_by_module_path(&self) -> Vec<&DerivedTarget>;
}

impl RegistryExt for ScanRegistry {
    fn group_by_attribute(
        &self,
        attr_key: &str,
    ) -> std::collections::HashMap<String, Vec<&DerivedTarget>> {
        let mut groups: std::collections::HashMap<String, Vec<&DerivedTarget>> =
            std::collections::HashMap::new();
        for target in self.values() {
            if let Some(value) = target.attributes.get(attr_key) {
                let group_key = match value {
                    AttributeValue::String(s) | AttributeValue::Ident(s) => s.clone(),
                    AttributeValue::Bool(b) => b.to_string(),
                    AttributeValue::Int(i) => i.to_string(),
                    _ => continue,
                };
                groups.entry(group_key).or_default().push(target);
            }
        }
        groups
    }

    fn filter_by_kind(&self, kind: &ItemKind) -> Vec<&DerivedTarget> {
        self.values().filter(|t| &t.item_kind == kind).collect()
    }

    fn filter_by_attribute(
        &self,
        attr_key: &str,
        attr_value: &AttributeValue,
    ) -> Vec<&DerivedTarget> {
        self.values()
            .filter(|t| t.attributes.get(attr_key) == Some(attr_value))
            .collect()
    }

    fn unique_attribute_values(&self, attr_key: &str) -> Vec<&AttributeValue> {
        let mut seen: Vec<&AttributeValue> = Vec::new();
        for target in self.values() {
            if let Some(value) = target.attributes.get(attr_key) {
                if !seen.contains(&value) {
                    seen.push(value);
                }
            }
        }
        seen
    }

    fn filter_by_crate(&self, crate_name: &str) -> Vec<&DerivedTarget> {
        self.values()
            .filter(|t| t.crate_name == crate_name)
            .collect()
    }

    fn sorted_by_module_path(&self) -> Vec<&DerivedTarget> {
        let mut targets: Vec<&DerivedTarget> = self.values().collect();
        targets.sort_by(|a, b| a.module_path.cmp(&b.module_path));
        targets
    }
}
