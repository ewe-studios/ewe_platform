//! Validates the `type` keyword.
//!
//! WHY: The `type` keyword restricts which JSON types are valid for a given
//! schema location.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};
use crate::types::{JsonType, JsonTypeSet};

use super::{Validate, ValidationContext};

pub struct TypeValidator {
    types: JsonTypeSet,
    schema_path: Location,
}

impl TypeValidator {
    pub fn new(types: JsonTypeSet, schema_path: Location) -> Self {
        Self { types, schema_path }
    }
}

impl Validate for TypeValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        let actual = JsonType::of(instance);
        if self.types.contains(actual) {
            return true;
        }
        // integer is a subset of number in JSON Schema
        actual == JsonType::Integer && self.types.contains(JsonType::Number)
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
            Err(
                ValidationErrorBuilder::new(instance_path.materialize(), self.schema_path.clone())
                    .build(ValidationErrorKind::InvalidType {
                        expected: self.types,
                        actual: JsonType::of(instance),
                    }),
            )
        }
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        match self.validate(instance, instance_path, ctx) {
            Ok(()) => Box::new(core::iter::empty()),
            Err(e) => Box::new(core::iter::once(e)),
        }
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
    fn string_match() {
        let v = TypeValidator::new(JsonTypeSet::with(JsonType::String), Location::new());
        assert!(v.is_valid(&json!("hello"), &mut ctx()));
    }

    #[test]
    fn string_mismatch() {
        let v = TypeValidator::new(JsonTypeSet::with(JsonType::String), Location::new());
        assert!(!v.is_valid(&json!(42), &mut ctx()));
    }

    #[test]
    fn integer_matches_number() {
        let v = TypeValidator::new(JsonTypeSet::with(JsonType::Number), Location::new());
        assert!(v.is_valid(&json!(42), &mut ctx()));
    }

    #[test]
    fn multi_type() {
        let mut types = JsonTypeSet::new();
        types.insert(JsonType::String);
        types.insert(JsonType::Null);
        let v = TypeValidator::new(types, Location::new());
        assert!(v.is_valid(&json!("hello"), &mut ctx()));
        assert!(v.is_valid(&json!(null), &mut ctx()));
        assert!(!v.is_valid(&json!(true), &mut ctx()));
    }
}
