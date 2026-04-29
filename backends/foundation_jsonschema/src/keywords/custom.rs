//! Custom keyword support via `KeywordFactory` trait.
//!
//! WHY: Users may need to add validation logic beyond the standard JSON Schema
//! keywords. This provides an extension point without forking the library.


use crate::error::ValidationError;
use crate::paths::Location;

use super::BoxedValidator;

/// Factory trait for creating custom keyword validators.
#[allow(dead_code)]
pub trait KeywordFactory: Send + Sync {
    /// Compile a keyword value into a validator.
    fn compile(
        &self,
        value: &serde_json::Value,
        schema_path: Location,
    ) -> Result<BoxedValidator, ValidationError>;
}
