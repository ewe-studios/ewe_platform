//! Validates `minLength` — string must have at least N Unicode characters.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct MinLengthValidator {
    min: u64,
    schema_path: Location,
}

impl MinLengthValidator {
    pub fn new(min: u64, schema_path: Location) -> Self {
        Self { min, schema_path }
    }
}

impl Validate for MinLengthValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if let Value::String(s) = instance {
            s.chars().count() as u64 >= self.min
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
            if len < self.min {
                return Err(ValidationErrorBuilder::new(
                    instance_path.materialize(),
                    self.schema_path.clone(),
                )
                .build(ValidationErrorKind::MinLength { min: self.min, actual: len }));
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
    fn valid_short() {
        let v = MinLengthValidator::new(2, Location::new());
        assert!(v.is_valid(&json!("ab"), &mut ctx()));
    }

    #[test]
    fn valid_unicode() {
        let v = MinLengthValidator::new(2, Location::new());
        assert!(v.is_valid(&json!("éà"), &mut ctx()));
    }

    #[test]
    fn invalid_too_short() {
        let v = MinLengthValidator::new(3, Location::new());
        assert!(!v.is_valid(&json!("ab"), &mut ctx()));
    }

    #[test]
    fn non_string_always_valid() {
        let v = MinLengthValidator::new(100, Location::new());
        assert!(v.is_valid(&json!(42), &mut ctx()));
    }
}
