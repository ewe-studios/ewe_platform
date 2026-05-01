//! `patternProperties` — regex-matched property validation.

use alloc::boxed::Box;
use alloc::vec::Vec;

use regex::Regex;
use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError};
use crate::node::SchemaNode;
use crate::paths::LazyLocation;

use super::{Validate, ValidationContext};

/// Validates properties whose names match regex patterns.
pub struct PatternPropertiesValidator {
    patterns: Vec<(Regex, SchemaNode)>,
}

impl PatternPropertiesValidator {
    /// Create with compiled regex → schema pairs.
    #[must_use]
    pub fn new(patterns: Vec<(Regex, SchemaNode)>) -> Self {
        Self { patterns }
    }
}

impl Validate for PatternPropertiesValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        if let Value::Object(obj) = instance {
            for (name, value) in obj {
                for (regex, schema) in &self.patterns {
                    if regex.is_match(name) {
                        // Mark as evaluated regardless of validation outcome.
                        ctx.mark_property_evaluated(name);
                        if !schema.is_valid(value, ctx) {
                            return false;
                        }
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
            for (name, value) in obj {
                for (regex, schema) in &self.patterns {
                    if regex.is_match(name) {
                        let child_path = instance_path.push_property(name);
                        // Mark as evaluated regardless of validation outcome.
                        ctx.mark_property_evaluated(name);
                        schema.validate(value, &child_path, ctx)?;
                    }
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
                for (regex, schema) in &self.patterns {
                    if regex.is_match(name) {
                        let child_path = instance_path.push_property(name);
                        for e in schema.iter_errors(value, &child_path, ctx) {
                            errors.push(e);
                        }
                        ctx.mark_property_evaluated(name);
                    }
                }
            }
        }
        Box::new(errors.into_iter())
    }
}
