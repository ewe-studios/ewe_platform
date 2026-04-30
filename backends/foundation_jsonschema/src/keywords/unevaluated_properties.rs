//! `unevaluatedProperties` — validates properties not evaluated by sibling keywords.

use alloc::boxed::Box;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError};
use crate::node::SchemaNode;
use crate::paths::LazyLocation;

use super::{Validate, ValidationContext};

/// Validates properties that weren't evaluated by `properties`, `patternProperties`, or `additionalProperties`.
pub struct UnevaluatedPropertiesValidator {
    schema: SchemaNode,
}

impl UnevaluatedPropertiesValidator {
    /// Create with a pre-compiled sub-schema.
    #[must_use]
    pub fn new(schema: SchemaNode) -> Self {
        Self { schema }
    }
}

impl Validate for UnevaluatedPropertiesValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        if let Value::Object(obj) = instance {
            for name in obj.keys() {
                if !ctx.is_property_evaluated(name) {
                    let value = &obj[name];
                    if !self.schema.is_valid(value, ctx) {
                        return false;
                    }
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
        if let Value::Object(obj) = instance {
            for name in obj.keys() {
                if !ctx.is_property_evaluated(name) {
                    let value = &obj[name];
                    let child_path = instance_path.push_property(name);
                    self.schema.validate(value, &child_path, ctx)?;
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
        if let Value::Object(obj) = instance {
            for name in obj.keys() {
                if !ctx.is_property_evaluated(name) {
                    let value = &obj[name];
                    let child_path = instance_path.push_property(name);
                    for e in self.schema.iter_errors(value, &child_path, ctx) {
                        errors.push(e);
                    }
                }
            }
        }
        Box::new(errors.into_iter())
    }
}
