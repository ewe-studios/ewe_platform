//! Validates `propertyNames` — all property names must match a schema.

use alloc::boxed::Box;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError};
use crate::node::SchemaNode;
use crate::paths::LazyLocation;

use super::{Validate, ValidationContext};

/// Validates each property name against a schema.
pub struct PropertyNamesValidator {
    schema: SchemaNode,
}

impl PropertyNamesValidator {
    /// Create with a pre-compiled sub-schema.
    #[must_use]
    pub fn new(schema: SchemaNode) -> Self {
        Self { schema }
    }
}

impl Validate for PropertyNamesValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        if let Value::Object(obj) = instance {
            obj.keys().all(|key| {
                let name = Value::String(key.clone());
                self.schema.is_valid(&name, ctx)
            })
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
        if let Value::Object(obj) = instance {
            for key in obj.keys() {
                let name = Value::String(key.clone());
                let child_path = instance_path.push_property(key);
                self.schema.validate(&name, &child_path, ctx)?;
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
            for key in obj.keys() {
                let name = Value::String(key.clone());
                let child_path = instance_path.push_property(key);
                for e in self.schema.iter_errors(&name, &child_path, ctx) {
                    errors.push(e);
                }
            }
        }
        Box::new(errors.into_iter())
    }
}
