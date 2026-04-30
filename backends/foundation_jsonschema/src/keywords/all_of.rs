//! `allOf` — all sub-schemas must validate.

use alloc::boxed::Box;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError};
use crate::node::SchemaNode;
use crate::paths::LazyLocation;

use super::{Validate, ValidationContext};

/// Validates that the instance satisfies all sub-schemas in the `allOf` array.
pub struct AllOfValidator {
    schemas: Vec<SchemaNode>,
}

impl AllOfValidator {
    /// Create a new `AllOfValidator` with pre-compiled sub-schemas.
    #[must_use]
    pub fn new(schemas: Vec<SchemaNode>) -> Self {
        Self { schemas }
    }
}

impl Validate for AllOfValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        self.schemas.iter().all(|s| s.is_valid(instance, ctx))
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        for schema in &self.schemas {
            schema.validate(instance, instance_path, ctx)?;
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
        for schema in &self.schemas {
            for e in schema.iter_errors(instance, instance_path, ctx) {
                errors.push(e);
            }
        }
        Box::new(errors.into_iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> ValidationContext {
        ValidationContext::new()
    }

    #[test]
    fn all_of_all_valid() {
        let v = AllOfValidator::new(vec![SchemaNode::AlwaysValid, SchemaNode::AlwaysValid]);
        assert!(v.is_valid(&json!("anything"), &mut ctx()));
    }

    #[test]
    fn all_of_one_invalid() {
        let v = AllOfValidator::new(vec![
            SchemaNode::AlwaysValid,
            SchemaNode::AlwaysInvalid {
                schema_path: crate::paths::Location::new(),
            },
        ]);
        assert!(!v.is_valid(&json!("anything"), &mut ctx()));
    }

    #[test]
    fn all_of_empty() {
        let v = AllOfValidator::new(vec![]);
        assert!(v.is_valid(&json!("anything"), &mut ctx()));
    }

    #[test]
    fn all_of_collects_all_errors() {
        let v = AllOfValidator::new(vec![
            SchemaNode::AlwaysInvalid {
                schema_path: crate::paths::Location::new(),
            },
            SchemaNode::AlwaysInvalid {
                schema_path: crate::paths::Location::new(),
            },
        ]);
        let errors: Vec<_> = v
            .iter_errors(&json!("x"), &LazyLocation::new(), &mut ctx())
            .collect();
        assert_eq!(errors.len(), 2);
    }
}
