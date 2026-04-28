//! Validates `multipleOf` — number must be an integer multiple of a divisor.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct MultipleOfValidator {
    multiple: f64,
    schema_path: Location,
}

impl MultipleOfValidator {
    pub fn new(multiple: f64, schema_path: Location) -> Self {
        Self { multiple, schema_path }
    }
}

impl Validate for MultipleOfValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if let Value::Number(n) = instance {
            if let Some(v) = n.as_f64() {
                let quotient = v / self.multiple;
                return (quotient - quotient.round()).abs() < f64::EPSILON * quotient.abs().max(1.0);
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
        if let Value::Number(n) = instance {
            if let Some(v) = n.as_f64() {
                if !self.is_valid(instance, ctx) {
                    return Err(ValidationErrorBuilder::new(
                        instance_path.materialize(),
                        self.schema_path.clone(),
                    )
                    .build(ValidationErrorKind::MultipleOf {
                        multiple: self.multiple,
                        actual: v,
                    }));
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
    fn divisible() {
        let v = MultipleOfValidator::new(5.0, Location::new());
        assert!(v.is_valid(&json!(10), &mut ctx()));
    }

    #[test]
    fn not_divisible() {
        let v = MultipleOfValidator::new(5.0, Location::new());
        assert!(!v.is_valid(&json!(12), &mut ctx()));
    }

    #[test]
    fn float_multiple() {
        let v = MultipleOfValidator::new(0.5, Location::new());
        assert!(v.is_valid(&json!(2.5), &mut ctx()));
    }

    #[test]
    fn non_number_always_valid() {
        let v = MultipleOfValidator::new(5.0, Location::new());
        assert!(v.is_valid(&json!("hello"), &mut ctx()));
    }
}
