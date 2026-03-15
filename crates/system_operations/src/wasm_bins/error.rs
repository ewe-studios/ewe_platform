use std::path::PathBuf;

/// WHY: Callers need structured errors to provide actionable feedback
/// about what went wrong during WASM binary generation.
///
/// WHAT: All errors that can occur during WASM binary generation.
///
/// HOW: Enum with variants for each failure mode. Manual `Display` + `Error`
/// impls following the `foundation_codegen` pattern (no `derive_more::Error`
/// for variants wrapping `PathBuf`).
#[derive(Debug)]
pub enum WasmBinError {
    /// Crate directory doesn't exist
    CrateNotFound(PathBuf),

    /// No Cargo.toml in crate directory
    NoCargoToml(PathBuf),

    /// Crate has incompatible `[lib]` crate-type
    InvalidCrateType {
        crate_dir: PathBuf,
        crate_type: String,
    },

    /// Cargo.toml parse error
    CargoTomlParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    /// `foundation_codegen` scanning error
    ScanError(foundation_codegen::CodegenError),

    /// File I/O error
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    /// No entrypoints found in the crate
    NoEntrypoints(PathBuf),

    /// Entrypoint missing a required attribute
    MissingAttribute { function: String, attribute: String },

    /// Entrypoint attribute has wrong type
    InvalidAttributeType {
        function: String,
        attribute: String,
        expected: String,
    },
}

impl std::fmt::Display for WasmBinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CrateNotFound(p) => {
                write!(f, "crate directory not found: {}", p.display())
            }
            Self::NoCargoToml(p) => {
                write!(f, "no Cargo.toml found in {}", p.display())
            }
            Self::InvalidCrateType {
                crate_dir,
                crate_type,
            } => write!(
                f,
                "crate at {} has incompatible crate-type `{crate_type}` \
                 (expected cdylib, bin, or no [lib] section)",
                crate_dir.display()
            ),
            Self::CargoTomlParse { path, source } => {
                write!(
                    f,
                    "failed to parse Cargo.toml at {}: {source}",
                    path.display()
                )
            }
            Self::ScanError(e) => write!(f, "source scanning failed: {e}"),
            Self::Io { path, source } => {
                write!(f, "I/O error at {}: {source}", path.display())
            }
            Self::NoEntrypoints(p) => {
                write!(
                    f,
                    "no #[wasm_entrypoint] functions found in {}",
                    p.display()
                )
            }
            Self::MissingAttribute {
                function,
                attribute,
            } => write!(
                f,
                "function `{function}` is missing required attribute `{attribute}`"
            ),
            Self::InvalidAttributeType {
                function,
                attribute,
                expected,
            } => write!(
                f,
                "function `{function}` attribute `{attribute}` must be {expected}"
            ),
        }
    }
}

impl std::error::Error for WasmBinError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CargoTomlParse { source, .. } => Some(source),
            Self::ScanError(e) => Some(e),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<foundation_codegen::CodegenError> for WasmBinError {
    fn from(e: foundation_codegen::CodegenError) -> Self {
        Self::ScanError(e)
    }
}
