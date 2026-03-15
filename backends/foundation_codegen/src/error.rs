use core::fmt;
use std::path::PathBuf;

/// WHY: Groups all codegen failures into a single type so callers can match
/// on specific failure modes (missing files vs parse errors vs bad config).
///
/// WHAT: Enum of all error conditions the source scanner can produce.
///
/// HOW: Manual `Display` for `PathBuf` formatting (needs `.display()`),
/// manual `Error` for selective `source()` delegation on variants with
/// an inner error.
#[derive(Debug)]
pub enum CodegenError {
    /// IO error with the path that was being accessed
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    /// Rust source file failed to parse
    ParseError { path: PathBuf, message: String },

    /// Cargo.toml deserialization failed
    CargoTomlError {
        path: PathBuf,
        source: toml::de::Error,
    },

    /// Expected Cargo.toml not found at path
    MissingCargoToml(PathBuf),

    /// Cargo.toml exists but has no [package] section
    MissingPackageSection(PathBuf),

    /// [package] section exists but `name` field is missing
    MissingPackageName(PathBuf),

    /// Could not locate a src/ directory for the crate
    MissingSrcDir(PathBuf),
}

impl std::error::Error for CodegenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::CargoTomlError { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "IO error reading {}: {source}", path.display())
            }
            Self::ParseError { path, message } => {
                write!(
                    f,
                    "failed to parse Rust source file {}: {message}",
                    path.display()
                )
            }
            Self::CargoTomlError { path, source } => {
                write!(
                    f,
                    "failed to parse Cargo.toml at {}: {source}",
                    path.display()
                )
            }
            Self::MissingCargoToml(path) => {
                write!(f, "missing Cargo.toml at {}", path.display())
            }
            Self::MissingPackageSection(path) => {
                write!(
                    f,
                    "missing [package] section in Cargo.toml at {}",
                    path.display()
                )
            }
            Self::MissingPackageName(path) => {
                write!(
                    f,
                    "missing package.name in Cargo.toml at {}",
                    path.display()
                )
            }
            Self::MissingSrcDir(path) => {
                write!(
                    f,
                    "could not determine source directory for crate at {}",
                    path.display()
                )
            }
        }
    }
}

/// WHY: Reduces boilerplate for functions returning `CodegenError`.
///
/// WHAT: Type alias for `std::result::Result` with `CodegenError` as the error type.
pub type Result<T> = std::result::Result<T, CodegenError>;
