use std::path::PathBuf;

#[derive(Debug)]
pub enum SchemaGenError {
    CrateNotFound(PathBuf),
    NoCargoToml(PathBuf),
    CargoTomlParse {
        path: PathBuf,
        source: toml::de::Error,
    },
    ScanError(foundation_codegen::CodegenError),
    ParseError {
        path: PathBuf,
        message: String,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    NoSchemaStructs(PathBuf),
}

impl std::fmt::Display for SchemaGenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CrateNotFound(p) => write!(f, "crate directory not found: {}", p.display()),
            Self::NoCargoToml(p) => write!(f, "no Cargo.toml found in {}", p.display()),
            Self::CargoTomlParse { path, source } => {
                write!(f, "failed to parse Cargo.toml at {}: {source}", path.display())
            }
            Self::ScanError(e) => write!(f, "source scanning failed: {e}"),
            Self::ParseError { path, message } => {
                write!(f, "failed to parse {}: {message}", path.display())
            }
            Self::Io { path, source } => write!(f, "I/O error at {}: {source}", path.display()),
            Self::NoSchemaStructs(p) => {
                write!(
                    f,
                    "no structs with #[derive(ArrowSchema)] or #[derive(JsonSchema)] found in {}",
                    p.display()
                )
            }
        }
    }
}

impl std::error::Error for SchemaGenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CargoTomlParse { source, .. } => Some(source),
            Self::ScanError(e) => Some(e),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<foundation_codegen::CodegenError> for SchemaGenError {
    fn from(e: foundation_codegen::CodegenError) -> Self {
        Self::ScanError(e)
    }
}
