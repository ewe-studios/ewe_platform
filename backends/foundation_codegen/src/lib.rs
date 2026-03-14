pub mod cargo_toml;
pub mod crate_scanner;
pub mod error;
pub mod file_walker;
pub mod module_path;
pub mod parser;
pub mod registry;
pub mod scanner;
pub mod types;
pub mod visitor;

pub use crate_scanner::{CrateScanner, RegistryExt};
pub use error::{CodegenError, Result};
pub use module_path::ModulePathResolver;
pub use registry::ScanRegistry;
pub use types::{AttributeValue, CrateMetadata, DerivedTarget, FoundItem, ItemKind, Location};
