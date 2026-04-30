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
        let base_state = ctx.save_evaluation_state();
        let mut count = 0;
        let mut match_snapshot = None;

        for schema in &self.schemas {
            let state = ctx.save_evaluation_state();
            if schema.is_valid(instance, ctx) {
                count += 1;
                match_snapshot = Some(ctx.save_evaluation_state());
            }
            ctx.restore_evaluation_state(&state);
        }

        if count == 1 {
            if let Some(snap) = match_snapshot {
                ctx.restore_evaluation_state(&snap);
            }
            true
        } else {
            ctx.restore_evaluation_state(&base_state);
            false
        }
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        let base_state = ctx.save_evaluation_state();
        let mut count = 0;
        let mut match_snapshot = None;

        for schema in &self.schemas {
            let state = ctx.save_evaluation_state();
            if schema.is_valid(instance, ctx) {
                count += 1;
                match_snapshot = Some(ctx.save_evaluation_state());
            }
            ctx.restore_evaluation_state(&state);
        }

        if count == 1 {
            if let Some(snap) = match_snapshot {
                ctx.restore_evaluation_state(&snap);
            }
            return Ok(());
        }

        ctx.restore_evaluation_state(&base_state);
        let kind = if count == 0 {
            ValidationErrorKind::OneOfNotValid
        } else {
            ValidationErrorKind::OneOfMultipleValid
        };
        Err(ValidationErrorBuilder::new(instance_path.materialize(), Location::new()).build(kind))
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        let base_state = ctx.save_evaluation_state();
        let mut count = 0;
        let mut match_snapshot = None;

        for schema in &self.schemas {
            let state = ctx.save_evaluation_state();
            if schema.is_valid(instance, ctx) {
                count += 1;
                match_snapshot = Some(ctx.save_evaluation_state());
            }
            ctx.restore_evaluation_state(&state);
        }

        if count == 1 {
            if let Some(snap) = match_snapshot {
                ctx.restore_evaluation_state(&snap);
            }
            return Box::new(core::iter::empty());
        }
        ctx.restore_evaluation_state(&base_state);
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
