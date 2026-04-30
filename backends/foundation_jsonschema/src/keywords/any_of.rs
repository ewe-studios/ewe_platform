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
        let mut any_valid = false;
        let base_state = ctx.save_evaluation_state();
        for schema in &self.schemas {
            let state = ctx.save_evaluation_state();
            if schema.is_valid(instance, ctx) {
                any_valid = true;
                // Keep marks from this matching branch, continue to collect more
            } else {
                ctx.restore_evaluation_state(&state);
            }
        }
        if !any_valid {
            ctx.restore_evaluation_state(&base_state);
        }
        any_valid
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        let mut all_errors: Vec<ValidationError> = Vec::new();
        let mut any_valid = false;
        let base_state = ctx.save_evaluation_state();

        for schema in &self.schemas {
            let state = ctx.save_evaluation_state();
            let result = schema.validate(instance, instance_path, ctx);
            if result.is_ok() {
                any_valid = true;
            } else {
                ctx.restore_evaluation_state(&state);
                if let Err(e) = result {
                    all_errors.push(e);
                }
            }
        }

        if any_valid {
            Ok(())
        } else {
            ctx.restore_evaluation_state(&base_state);
            Err(all_errors.into_iter().next().unwrap_or_else(|| {
                ValidationErrorBuilder::new(instance_path.materialize(), Location::new())
                    .build(ValidationErrorKind::AnyOf)
            }))
        }
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        let base_state = ctx.save_evaluation_state();
        let mut any_valid = false;
        for schema in &self.schemas {
            let state = ctx.save_evaluation_state();
            if schema.is_valid(instance, ctx) {
                any_valid = true;
            } else {
                ctx.restore_evaluation_state(&state);
            }
        }
        if any_valid {
            return Box::new(core::iter::empty());
        }
        ctx.restore_evaluation_state(&base_state);
        let err = ValidationErrorBuilder::new(instance_path.materialize(), Location::new())
            .build(ValidationErrorKind::AnyOf);
        Box::new(core::iter::once(err))
    }
}
