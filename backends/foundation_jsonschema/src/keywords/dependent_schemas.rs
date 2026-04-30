//! `dependentSchemas` — property presence triggers schema validation.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError};
use crate::node::SchemaNode;
use crate::paths::LazyLocation;

use super::{Validate, ValidationContext};

/// Validates that if a property is present, its dependent schema also validates.
pub struct DependentSchemasValidator {
    dependencies: BTreeMap<String, SchemaNode>,
}

impl DependentSchemasValidator {
    /// Create with a map of property name → dependent schema.
    #[must_use]
    pub fn new(dependencies: BTreeMap<String, SchemaNode>) -> Self {
        Self { dependencies }
    }
}

impl Validate for DependentSchemasValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        if let Value::Object(obj) = instance {
            for (name, schema) in &self.dependencies {
                if obj.contains_key(name) && !schema.is_valid(instance, ctx) {
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
        if let Value::Object(obj) = instance {
            for (name, schema) in &self.dependencies {
                if obj.contains_key(name) {
                    schema.validate(instance, instance_path, ctx)?;
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
            for (name, schema) in &self.dependencies {
                if obj.contains_key(name) {
                    for e in schema.iter_errors(instance, instance_path, ctx) {
                        errors.push(e);
                    }
                }
            }
        }
        Box::new(errors.into_iter())
    }
}
