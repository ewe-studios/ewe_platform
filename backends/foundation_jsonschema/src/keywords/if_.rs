//! `if`/`then`/`else` conditional validation.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError};
use crate::node::SchemaNode;
use crate::paths::LazyLocation;

use super::{Validate, ValidationContext};

/// Conditional validation: if the `if` schema validates, then `then` must
/// also validate; otherwise `else` must validate.
#[allow(clippy::struct_field_names)]
pub struct IfThenElseValidator {
    if_schema: SchemaNode,
    then_schema: Option<SchemaNode>,
    else_schema: Option<SchemaNode>,
}

impl IfThenElseValidator {
    /// Create with pre-compiled sub-schemas.
    #[must_use]
    pub fn new(
        if_schema: SchemaNode,
        then_schema: Option<SchemaNode>,
        else_schema: Option<SchemaNode>,
    ) -> Self {
        Self {
            if_schema,
            then_schema,
            else_schema,
        }
    }
}

impl Validate for IfThenElseValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        let pre_if = ctx.save_evaluation_state();
        let condition = self.if_schema.is_valid(instance, ctx);

        if condition {
            // `if` passed — keep its marks, run `then`
            self.then_schema
                .as_ref()
                .is_none_or(|s| s.is_valid(instance, ctx))
        } else {
            // `if` failed — discard its marks, run `else`
            ctx.restore_evaluation_state(&pre_if);
            self.else_schema
                .as_ref()
                .is_none_or(|s| s.is_valid(instance, ctx))
        }
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        let pre_if = ctx.save_evaluation_state();
        let condition = self.if_schema.is_valid(instance, ctx);

        if condition {
            if let Some(then_schema) = &self.then_schema {
                then_schema.validate(instance, instance_path, ctx)?;
            }
        } else {
            ctx.restore_evaluation_state(&pre_if);
            if let Some(else_schema) = &self.else_schema {
                else_schema.validate(instance, instance_path, ctx)?;
            }
        }
        Ok(())
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        let pre_if = ctx.save_evaluation_state();
        let condition = self.if_schema.is_valid(instance, ctx);

        if condition {
            match &self.then_schema {
                Some(s) => s.iter_errors(instance, instance_path, ctx),
                None => Box::new(core::iter::empty()),
            }
        } else {
            ctx.restore_evaluation_state(&pre_if);
            match &self.else_schema {
                Some(s) => s.iter_errors(instance, instance_path, ctx),
                None => Box::new(core::iter::empty()),
            }
        }
    }
}
