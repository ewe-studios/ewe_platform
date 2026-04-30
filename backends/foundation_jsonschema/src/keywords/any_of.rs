//! `anyOf` — at least one sub-schema must validate.

use alloc::boxed::Box;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::node::SchemaNode;
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

/// Validates that the instance satisfies at least one sub-schema.
pub struct AnyOfValidator {
    schemas: Vec<SchemaNode>,
}

impl AnyOfValidator {
    /// Create with pre-compiled sub-schemas.
    #[must_use]
    pub fn new(schemas: Vec<SchemaNode>) -> Self {
        Self { schemas }
    }
}

impl Validate for AnyOfValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        self.schemas.iter().any(|s| s.is_valid(instance, ctx))
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        if self.schemas.iter().any(|s| s.is_valid(instance, ctx)) {
            return Ok(());
        }
        Err(
            ValidationErrorBuilder::new(instance_path.materialize(), Location::new())
                .build(ValidationErrorKind::AnyOf),
        )
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        if self.schemas.iter().any(|s| s.is_valid(instance, ctx)) {
            return Box::new(core::iter::empty());
        }
        let err = ValidationErrorBuilder::new(instance_path.materialize(), Location::new())
            .build(ValidationErrorKind::AnyOf);
        Box::new(core::iter::once(err))
    }
}
