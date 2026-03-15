pub mod error;
mod generator;
mod planner;
pub mod validator;

use std::path::{Path, PathBuf};

use foundation_codegen::{AttributeValue, CrateScanner, ItemKind, RegistryExt};

use error::WasmBinError;

/// WHY: Centralizes WASM binary generation logic so it can be used
/// from the CLI, tests, and build scripts without duplication.
///
/// WHAT: Scans a crate for `#[wasm_entrypoint]` functions and generates
/// binary source files + Cargo.toml `[[bin]]` entries for WASM compilation.
///
/// HOW: Uses `foundation_codegen::CrateScanner` to find annotated functions,
/// validates the crate structure, then generates binary files.
pub struct WasmBinGenerator {
    entrypoints: Vec<WasmEntrypoint>,
    crate_dir: PathBuf,
    crate_name: String,
}

/// WHY: Each discovered entrypoint carries metadata needed for generating
/// binary files and displaying information to the user.
///
/// WHAT: A discovered WASM entrypoint with its metadata.
///
/// HOW: Extracted from `DerivedTarget` by reading `name` and `desc` attributes.
#[derive(Debug, Clone)]
pub struct WasmEntrypoint {
    /// Binary name from `name` attribute (e.g., `auth_worker`)
    pub name: String,
    /// Description from `desc` attribute
    pub description: String,
    /// The function name in source (e.g., `auth_handler`)
    pub function_name: String,
    /// Full module path to the function (e.g., `my_crate::handlers::auth_handler`)
    pub qualified_path: String,
    /// Source file location
    pub source_file: PathBuf,
    /// Line number in source
    pub line: usize,
}

/// WHY: The CLI needs a preview of what `generate` would do without actually
/// writing files.
///
/// WHAT: Result of a dry-run scan — lists what would be generated.
///
/// HOW: Built by the planner from discovered entrypoints.
#[derive(Debug, Clone)]
pub struct WasmBinPlan {
    /// Crate name
    pub crate_name: String,
    /// Crate directory
    pub crate_dir: PathBuf,
    /// Discovered entrypoints
    pub entrypoints: Vec<WasmEntrypoint>,
    /// `[[bin]]` sections that would be added to Cargo.toml
    pub bin_sections: Vec<BinSection>,
    /// Generated file paths and their content
    pub generated_files: Vec<GeneratedFile>,
    /// Expected WASM output paths by build profile
    pub wasm_outputs: Vec<WasmOutput>,
}

/// WHY: The Cargo.toml needs `[[bin]]` entries so cargo knows about the binaries.
///
/// WHAT: A `[[bin]]` entry for Cargo.toml.
///
/// HOW: Simple name + path pair.
#[derive(Debug, Clone)]
pub struct BinSection {
    pub name: String,
    pub path: String,
}

/// WHY: The user needs to see what files will be created before committing.
///
/// WHAT: A file that will be or was generated.
///
/// HOW: Path relative to crate root + file content.
#[derive(Debug, Clone)]
pub struct GeneratedFile {
    /// Path relative to crate root (e.g., `bin/auth_worker/main.rs`)
    pub path: PathBuf,
    /// File content
    pub content: String,
}

/// WHY: Users need to know where the compiled WASM files will end up.
///
/// WHAT: Expected WASM output location for a binary.
///
/// HOW: Computed from the binary name and standard cargo output paths.
#[derive(Debug, Clone)]
pub struct WasmOutput {
    pub bin_name: String,
    pub debug_path: String,
    pub release_path: String,
}

impl WasmBinGenerator {
    /// WHY: The generator needs to scan and validate the crate upfront so
    /// both `plan()` and `generate()` work from the same discovered state.
    ///
    /// WHAT: Creates a new generator by scanning the given crate directory
    /// for `#[wasm_entrypoint]` functions.
    ///
    /// HOW: Validates the crate structure, runs `CrateScanner`, and extracts
    /// entrypoint metadata from discovered `DerivedTarget`s.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Directory doesn't exist or has no Cargo.toml
    /// - Crate fails validation (wrong crate-type)
    /// - Source scanning fails
    /// - An entrypoint is missing required `name` or `desc` attributes
    pub fn new(crate_dir: &Path) -> Result<Self, WasmBinError> {
        let cargo_toml = validator::validate_crate(crate_dir)?;

        let crate_name = cargo_toml
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(toml::Value::as_str)
            .unwrap_or("unknown")
            .to_string();

        let scanner = CrateScanner::new("wasm_entrypoint");
        let registry = scanner.scan_crate(crate_dir)?;

        // Filter to functions only
        let functions = registry.filter_by_kind(&ItemKind::Function);

        let mut entrypoints = Vec::with_capacity(functions.len());

        for target in functions {
            let name = extract_string_attr(target, "name")?;
            let description = extract_string_attr(target, "desc")?;

            entrypoints.push(WasmEntrypoint {
                name,
                description,
                function_name: target.item_name.clone(),
                qualified_path: target.qualified_path.clone(),
                source_file: target.location.file_path.clone(),
                line: target.location.line,
            });
        }

        Ok(Self {
            entrypoints,
            crate_dir: crate_dir.to_path_buf(),
            crate_name,
        })
    }

    /// WHY: The `list` subcommand needs a preview without side effects.
    ///
    /// WHAT: Performs a dry-run: scans and plans what would be generated,
    /// without writing any files.
    ///
    /// HOW: Delegates to the planner module.
    ///
    /// # Errors
    ///
    /// Returns error if no entrypoints were found.
    pub fn plan(&self) -> Result<WasmBinPlan, WasmBinError> {
        planner::build_plan(&self.crate_name, &self.crate_dir, &self.entrypoints)
    }

    /// WHY: The `generate` subcommand needs to create actual files.
    ///
    /// WHAT: Executes generation: creates binary files and updates Cargo.toml.
    ///
    /// HOW: Builds a plan, then writes files via the generator module.
    ///
    /// # Errors
    ///
    /// Returns error if file writing or Cargo.toml modification fails.
    pub fn generate(&self) -> Result<WasmBinPlan, WasmBinError> {
        let plan = self.plan()?;
        generator::execute_plan(&plan)?;
        Ok(plan)
    }

    /// WHY: CLI formatting needs access to the crate name.
    ///
    /// WHAT: Returns the crate name.
    ///
    /// HOW: Returns a reference to the stored crate name.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    /// WHY: CLI formatting needs access to the entrypoints.
    ///
    /// WHAT: Returns the discovered entrypoints.
    ///
    /// HOW: Returns a reference to the stored entrypoints.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn entrypoints(&self) -> &[WasmEntrypoint] {
        &self.entrypoints
    }
}

/// WHY: Multiple entrypoint fields need the same extraction logic.
///
/// WHAT: Extracts a string attribute from a `DerivedTarget`.
///
/// HOW: Looks up the attribute key and verifies it's a `String` variant.
///
/// # Errors
///
/// Returns `WasmBinError::MissingAttribute` if the key is not found.
/// Returns `WasmBinError::InvalidAttributeType` if the value is not a string.
fn extract_string_attr(
    target: &foundation_codegen::DerivedTarget,
    attr_key: &str,
) -> Result<String, WasmBinError> {
    match target.attributes.get(attr_key) {
        Some(AttributeValue::String(s)) => Ok(s.clone()),
        Some(_) => Err(WasmBinError::InvalidAttributeType {
            function: target.item_name.clone(),
            attribute: attr_key.to_string(),
            expected: "a string literal".to_string(),
        }),
        None => Err(WasmBinError::MissingAttribute {
            function: target.item_name.clone(),
            attribute: attr_key.to_string(),
        }),
    }
}
