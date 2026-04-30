//! `not` — the instance must NOT validate against the sub-schema.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::node::SchemaNode;
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

/// Validates that the instance does NOT satisfy the sub-schema.
pub struct NotValidator {
    schema: SchemaNode,
}

impl NotValidator {
    /// Create with a pre-compiled sub-schema.
    #[must_use]
    pub fn new(schema: SchemaNode) -> Self {
        Self { schema }
    }
}

impl Validate for NotValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        let state = ctx.save_evaluation_state();
        let result = !self.schema.is_valid(instance, ctx);
        ctx.restore_evaluation_state(&state);
        result
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        let state = ctx.save_evaluation_state();
        let inner_valid = self.schema.is_valid(instance, ctx);
        ctx.restore_evaluation_state(&state);
        if inner_valid {
            return Err(
                ValidationErrorBuilder::new(instance_path.materialize(), Location::new())
                    .build(ValidationErrorKind::Not),
            );
        }
        Ok(())
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        let state = ctx.save_evaluation_state();
        let inner_valid = self.schema.is_valid(instance, ctx);
        ctx.restore_evaluation_state(&state);
        if inner_valid {
            let err = ValidationErrorBuilder::new(instance_path.materialize(), Location::new())
                .build(ValidationErrorKind::Not);
            Box::new(core::iter::once(err))
        } else {
            Box::new(core::iter::empty())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keywords::type_::TypeValidator;
    use crate::types::JsonTypeSet;
    use serde_json::json;

    fn ctx() -> ValidationContext {
        ValidationContext::new()
    }

    #[test]
    fn not_valid_instance() {
        let mut types = JsonTypeSet::new();
        types.insert(crate::types::JsonType::String);
        let schema = SchemaNode::Validators {
            validators: vec![Box::new(TypeValidator::new(types, Location::new()))],
            schema_path: Location::new(),
        };
        let v = NotValidator::new(schema);
        assert!(v.is_valid(&json!(42), &mut ctx())); // number is NOT a string → valid
    }

    #[test]
    fn not_invalid_instance() {
        let mut types = JsonTypeSet::new();
        types.insert(crate::types::JsonType::String);
        let schema = SchemaNode::Validators {
            validators: vec![Box::new(TypeValidator::new(types, Location::new()))],
            schema_path: Location::new(),
        };
        let v = NotValidator::new(schema);
        assert!(!v.is_valid(&json!("hello"), &mut ctx())); // string IS a string → invalid
    }

    #[test]
    fn not_against_always_valid() {
        let v = NotValidator::new(SchemaNode::AlwaysValid);
        assert!(!v.is_valid(&json!("anything"), &mut ctx()));
    }

    #[test]
    fn not_against_always_invalid() {
        let v = NotValidator::new(SchemaNode::AlwaysInvalid {
            schema_path: Location::new(),
        });
        assert!(v.is_valid(&json!("anything"), &mut ctx()));
    }
}
