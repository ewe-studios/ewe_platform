//! `contains` — at least one array item must validate against the sub-schema.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::node::SchemaNode;
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

/// Validates that at least one array item satisfies the sub-schema.
pub struct ContainsValidator {
    schema: SchemaNode,
}

impl ContainsValidator {
    /// Create with a pre-compiled sub-schema.
    #[must_use]
    pub fn new(schema: SchemaNode) -> Self {
        Self { schema }
    }
}

impl Validate for ContainsValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        if let Value::Array(arr) = instance {
            arr.iter().any(|item| self.schema.is_valid(item, ctx))
        } else {
            true
        }
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
                ValidationErrorBuilder::new(instance_path.materialize(), Location::new())
                    .build(ValidationErrorKind::Contains),
            )
        }
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        if self.is_valid(instance, ctx) {
            Box::new(core::iter::empty())
        } else {
            let err = ValidationErrorBuilder::new(instance_path.materialize(), Location::new())
                .build(ValidationErrorKind::Contains);
            Box::new(core::iter::once(err))
        }
    }
}
