use std::collections::HashMap;

use crate::types::DerivedTarget;

/// WHY: Consumers need a type-safe alias for the scan registry that clearly
/// communicates its purpose and structure.
///
/// WHAT: Type alias for the scan results: a `HashMap` mapping qualified paths
/// to `DerivedTarget` metadata.
///
/// HOW: Uses `String` keys (qualified paths like `crate::module::ItemName`)
/// to avoid name collisions across crates.
pub type ScanRegistry = HashMap<String, DerivedTarget>;
