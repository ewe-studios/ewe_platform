//! `format` keyword — validates string format when `assert_format` is enabled.

use alloc::boxed::Box;
use alloc::string::String;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::formats::FormatChecker;
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

/// Validates string format when `assert_mode` is enabled.
pub struct FormatValidator {
    format_name: String,
    schema_path: Location,
    checker: Option<Box<dyn FormatChecker>>,
    assert_mode: bool,
}

impl FormatValidator {
    /// Create a new format validator.
    ///
    /// If `checker` is `None`, the format is unknown and validation always passes
    /// (per JSON Schema spec, unknown formats are annotations only).
    /// If `assert_mode` is `false`, format is annotation-only regardless of checker.
    #[must_use]
    pub fn new(
        format_name: String,
        schema_path: Location,
        checker: Option<Box<dyn FormatChecker>>,
        assert_mode: bool,
    ) -> Self {
        Self {
            format_name,
            schema_path,
            checker,
            assert_mode,
        }
    }
}

impl Validate for FormatValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if !self.assert_mode {
            return true;
        }
        if let Value::String(s) = instance {
            if let Some(ref checker) = self.checker {
                return checker.check(s);
            }
        }
        true
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        if self.is_valid(instance, ctx) {
            Ok(())
        } else {
            Err(
                ValidationErrorBuilder::new(instance_path.materialize(), self.schema_path.clone())
                    .build(ValidationErrorKind::Format {
                        format: self.format_name.clone(),
                    }),
            )
        }
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        match self.validate(instance, instance_path, ctx) {
            Ok(()) => Box::new(core::iter::empty()),
            Err(e) => Box::new(core::iter::once(e)),
        }
    }
}
