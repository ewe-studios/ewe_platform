//! Custom keyword support via `KeywordFactory` trait.
//!
//! WHY: Users may need to add validation logic beyond the standard JSON Schema
//! keywords. This provides an extension point without forking the library.

use crate::error::ValidationError;
use crate::paths::Location;

use super::BoxedValidator;

/// Factory trait for creating custom keyword validators.
pub trait KeywordFactory: Send + Sync {
    /// Compile a keyword value into a validator.
    ///
    /// # Errors
    ///
    /// Returns an error if the keyword value is malformed or cannot be compiled.
    fn compile(
        &self,
        value: &serde_json::Value,
        schema_path: Location,
    ) -> Result<BoxedValidator, ValidationError>;
    /// Clone this factory into a boxed trait object.
    fn clone_box(&self) -> Box<dyn KeywordFactory>;
}

impl Clone for Box<dyn KeywordFactory> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
