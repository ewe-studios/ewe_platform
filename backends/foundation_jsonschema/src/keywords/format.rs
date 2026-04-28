//! Stub: `format` dispatcher — will be implemented in Feature 7.

use alloc::boxed::Box;
use alloc::string::String;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct FormatValidator {
    format_name: String,
    schema_path: Location,
}

impl FormatValidator {
    pub fn new(format_name: String, schema_path: Location) -> Self {
        Self { format_name, schema_path }
    }
}

impl Validate for FormatValidator {
    fn is_valid(&self, _instance: &Value, _ctx: &mut ValidationContext) -> bool {
        // TODO: Feature 7 — actual format validation
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
            Err(ValidationErrorBuilder::new(
                instance_path.materialize(),
                self.schema_path.clone(),
            )
            .build(ValidationErrorKind::Format {
                format: self.format_name.clone(),
            }))
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
