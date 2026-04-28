//! Custom keyword support via `KeywordFactory` trait.
//!
//! WHY: Users may need to add validation logic beyond the standard JSON Schema
//! keywords. This provides an extension point without forking the library.

use alloc::boxed::Box;

use crate::error::ValidationError;
use crate::paths::Location;

use super::BoxedValidator;

/// Factory trait for creating custom keyword validators.
///
/// WHY: The compiler needs a way to create validators for user-defined keywords
/// without knowing the concrete type at compile time.
pub trait KeywordFactory: Send + Sync {
    /// Compile a keyword value into a validator.
    fn compile(
        &self,
        value: &serde_json::Value,
        schema_path: Location,
    ) -> Result<BoxedValidator, ValidationError>;
}
