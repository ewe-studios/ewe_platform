//! Validates `maxLength` — string must have at most N Unicode characters.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct MaxLengthValidator {
    max: u64,
    schema_path: Location,
}

impl MaxLengthValidator {
    pub fn new(max: u64, schema_path: Location) -> Self {
        Self { max, schema_path }
    }
}

impl Validate for MaxLengthValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if let Value::String(s) = instance {
            s.chars().count() as u64 <= self.max
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
        if let Value::String(s) = instance {
            let len = s.chars().count() as u64;
            if len > self.max {
                return Err(ValidationErrorBuilder::new(
                    instance_path.materialize(),
                    self.schema_path.clone(),
                )
                .build(ValidationErrorKind::MaxLength { max: self.max, actual: len }));
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
    fn valid_length() {
        let v = MaxLengthValidator::new(3, Location::new());
        assert!(v.is_valid(&json!("abc"), &mut ctx()));
    }

    #[test]
    fn valid_shorter() {
        let v = MaxLengthValidator::new(3, Location::new());
        assert!(v.is_valid(&json!("a"), &mut ctx()));
    }

    #[test]
    fn invalid_too_long() {
        let v = MaxLengthValidator::new(2, Location::new());
        assert!(!v.is_valid(&json!("abc"), &mut ctx()));
    }

    #[test]
    fn non_string_always_valid() {
        let v = MaxLengthValidator::new(0, Location::new());
        assert!(v.is_valid(&json!(42), &mut ctx()));
    }
}
