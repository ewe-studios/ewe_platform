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
        if self.if_schema.is_valid(instance, ctx) {
            self.then_schema
                .as_ref()
                .is_none_or(|s| s.is_valid(instance, ctx))
        } else {
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
        if self.is_valid(instance, ctx) {
            Ok(())
        } else {
            let schema = if self.if_schema.is_valid(instance, ctx) {
                &self.then_schema
            } else {
                &self.else_schema
            };
            if let Some(s) = schema {
                s.validate(instance, instance_path, ctx)
            } else {
                Ok(())
            }
        }
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        if self.is_valid(instance, ctx) {
            return Box::new(core::iter::empty());
        }
        let schema = if self.if_schema.is_valid(instance, ctx) {
            &self.then_schema
        } else {
            &self.else_schema
        };
        match schema {
            Some(s) => Box::new(s.iter_errors(instance, instance_path, ctx)),
            None => Box::new(core::iter::empty()),
        }
    }
}
