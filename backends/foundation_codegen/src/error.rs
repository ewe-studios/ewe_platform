use std::path::PathBuf;

use derive_more::{Display, Error};
use foundation_errstacks::ErrorTrace;

/// WHY: Groups all codegen failures into a single type so callers can match
/// on specific failure modes (missing files vs parse errors vs bad config).
///
/// WHAT: Enum of all error conditions the source scanner can produce, and the
/// [`Result`] alias over an errstacks trace.
///
/// HOW: `derive_more` supplies `Display` and `Error`; a `From` impl converts
/// the enum into an `ErrorTrace` at the crate boundary.
#[derive(Debug, Display, Error)]
pub enum CodegenError {
    /// IO error with the path that was being accessed
    #[display("IO error reading {}: {source}", path.display())]
    Io {
        /// The file or directory that was being accessed.
        path: PathBuf,
        /// The underlying I/O error.
        #[error(source)]
        source: std::io::Error,
    },

    /// Rust source file failed to parse
    #[display("failed to parse Rust source file {}: {message}", path.display())]
    ParseError {
        /// The file that failed to parse.
        path: PathBuf,
        /// The parser message.
        message: String,
    },

    /// Cargo.toml deserialization failed
    #[display("failed to parse Cargo.toml at {}: {source}", path.display())]
    CargoTomlError {
        /// The manifest path.
        path: PathBuf,
        /// The TOML deserialization error.
        #[error(source)]
        source: toml::de::Error,
    },

    /// Expected Cargo.toml not found at path
    #[display("missing Cargo.toml at {}", _0.display())]
    MissingCargoToml(#[error(not(source))] PathBuf),

    /// Cargo.toml exists but has no [package] section
    #[display("missing [package] section in Cargo.toml at {}", _0.display())]
    MissingPackageSection(#[error(not(source))] PathBuf),

    /// [package] section exists but `name` field is missing
    #[display("missing package.name in Cargo.toml at {}", _0.display())]
    MissingPackageName(#[error(not(source))] PathBuf),

    /// Could not locate a src/ directory for the crate
    #[display("could not determine source directory for crate at {}", _0.display())]
    MissingSrcDir(#[error(not(source))] PathBuf),
}

impl From<CodegenError> for ErrorTrace<CodegenError> {
    fn from(err: CodegenError) -> Self {
        ErrorTrace::new(err)
    }
}

/// WHY: Reduces boilerplate for functions returning [`CodegenError`].
///
/// WHAT: Type alias for `std::result::Result` with an errstacks trace as the
/// error type.
pub type Result<T> = std::result::Result<T, ErrorTrace<CodegenError>>;
