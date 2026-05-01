//! Validates the `const` keyword.
//!
//! WHY: The `const` keyword requires the instance to be exactly equal to a
//! specific value.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct ConstValidator {
    expected: Value,
    schema_path: Location,
}

impl ConstValidator {
    pub fn new(expected: Value, schema_path: Location) -> Self {
        Self {
            expected,
            schema_path,
        }
    }
}

impl Validate for ConstValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        values_equal(instance, &self.expected)
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
                    .build(ValidationErrorKind::InvalidConst {
                        expected: self.expected.clone(),
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

/// JSON Schema equality: integer 1 equals float 1.0
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            // Same integer representation: exact comparison.
            if a.as_i64().is_some() && b.as_i64().is_some() {
                return a.as_i64() == b.as_i64();
            }
            if a.as_u64().is_some() && b.as_u64().is_some() {
                return a.as_u64() == b.as_u64();
            }
            // Cross-representation: fall back to f64 (handles 0 == 0.0).
            a.as_f64() == b.as_f64()
        }
        _ => a == b,
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
    fn match_string() {
        let v = ConstValidator::new(json!("hello"), Location::new());
        assert!(v.is_valid(&json!("hello"), &mut ctx()));
    }

    #[test]
    fn mismatch_string() {
        let v = ConstValidator::new(json!("hello"), Location::new());
        assert!(!v.is_valid(&json!("world"), &mut ctx()));
    }

    #[test]
    fn int_equals_float() {
        let v = ConstValidator::new(json!(1), Location::new());
        assert!(v.is_valid(&json!(1.0), &mut ctx()));
    }
}
