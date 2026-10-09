//! `oneOf` — exactly one sub-schema must validate.

use alloc::boxed::Box;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::node::SchemaNode;
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

/// Validates that the instance satisfies exactly one sub-schema.
pub struct OneOfValidator {
    schemas: Vec<SchemaNode>,
}

impl OneOfValidator {
    /// Create with pre-compiled sub-schemas.
    #[must_use]
    pub fn new(schemas: Vec<SchemaNode>) -> Self {
        Self { schemas }
    }
}

impl Validate for OneOfValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        let mut count = 0;

        for schema in &self.schemas {
            if schema.is_valid(instance, ctx) {
                count += 1;
            }
        }

        count == 1
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        let mut count = 0;

        for schema in &self.schemas {
            if schema.is_valid(instance, ctx) {
                count += 1;
            }
        }

        if count == 1 {
            Ok(())
        } else {
            let kind = if count == 0 {
                ValidationErrorKind::OneOfNotValid
            } else {
                ValidationErrorKind::OneOfMultipleValid
            };
            Err(
                ValidationErrorBuilder::new(instance_path.materialize(), Location::new())
                    .build(kind),
            )
        }
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        let mut count = 0;

        for schema in &self.schemas {
            if schema.is_valid(instance, ctx) {
                count += 1;
            }
        }

        if count == 1 {
            return Box::new(core::iter::empty());
        }
        let kind = if count == 0 {
            ValidationErrorKind::OneOfNotValid
        } else {
            ValidationErrorKind::OneOfMultipleValid
        };
        let err =
            ValidationErrorBuilder::new(instance_path.materialize(), Location::new()).build(kind);
        Box::new(core::iter::once(err))
    }
}
