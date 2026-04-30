//! `unevaluatedItems` — validates array items not evaluated by sibling keywords.

use alloc::boxed::Box;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError};
use crate::node::SchemaNode;
use crate::paths::LazyLocation;

use super::{Validate, ValidationContext};

/// Validates array items that weren't evaluated by `items` or `prefixItems`.
pub struct UnevaluatedItemsValidator {
    schema: SchemaNode,
}

impl UnevaluatedItemsValidator {
    /// Create with a pre-compiled sub-schema.
    #[must_use]
    pub fn new(schema: SchemaNode) -> Self {
        Self { schema }
    }
}

impl Validate for UnevaluatedItemsValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        if let Value::Array(arr) = instance {
            for (i, item) in arr.iter().enumerate() {
                if !ctx.is_item_evaluated(i) && !self.schema.is_valid(item, ctx) {
                    return false;
                }
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
        if let Value::Array(arr) = instance {
            for (i, item) in arr.iter().enumerate() {
                if !ctx.is_item_evaluated(i) {
                    let child_path = instance_path.push_index(i);
                    self.schema.validate(item, &child_path, ctx)?;
                }
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
        let mut errors: Vec<ValidationError> = Vec::new();
        if let Value::Array(arr) = instance {
            for (i, item) in arr.iter().enumerate() {
                if !ctx.is_item_evaluated(i) {
                    let child_path = instance_path.push_index(i);
                    for e in self.schema.iter_errors(item, &child_path, ctx) {
                        errors.push(e);
                    }
                }
            }
        }
        Box::new(errors.into_iter())
    }
}
