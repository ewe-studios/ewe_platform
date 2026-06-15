//! `properties` — validates named properties against their sub-schemas.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError};
use crate::node::SchemaNode;
use crate::paths::LazyLocation;

use super::{Validate, ValidationContext};

/// Validates each named property against its compiled sub-schema.
pub struct PropertiesValidator {
    properties: BTreeMap<String, SchemaNode>,
}

impl PropertiesValidator {
    /// Create with a map of property name → compiled schema.
    #[must_use]
    pub fn new(properties: BTreeMap<String, SchemaNode>) -> Self {
        Self { properties }
    }
}

impl Validate for PropertiesValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        let mut valid = true;
        if let Value::Object(obj) = instance {
            for (name, value) in obj {
                if let Some(schema) = self.properties.get(name) {
                    let state = ctx.save_evaluation_state();
                    ctx.mark_property_evaluated(name);
                    if !schema.is_valid(value, ctx) {
                        valid = false;
                    }
                    // Restore to pre-subschema state but keep our mark
                    ctx.restore_evaluation_state(&state);
                    ctx.mark_property_evaluated(name);
                }
            }
        }
        valid
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        if let Value::Object(obj) = instance {
            for (name, value) in obj {
                if let Some(schema) = self.properties.get(name) {
                    let child_path = instance_path.push_property(name);
                    let state = ctx.save_evaluation_state();
                    ctx.mark_property_evaluated(name);
                    schema.validate(value, &child_path, ctx)?;
                    ctx.restore_evaluation_state(&state);
                    ctx.mark_property_evaluated(name);
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
            for (name, value) in obj {
                if let Some(schema) = self.properties.get(name) {
                    let child_path = instance_path.push_property(name);
                    let state = ctx.save_evaluation_state();
                    ctx.mark_property_evaluated(name);
                    for e in schema.iter_errors(value, &child_path, ctx) {
                        errors.push(e);
                    }
                    ctx.restore_evaluation_state(&state);
                    ctx.mark_property_evaluated(name);
                }
            }
        }
        Box::new(errors.into_iter())
    }
}
