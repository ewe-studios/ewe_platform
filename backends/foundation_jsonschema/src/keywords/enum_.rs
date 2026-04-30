//! Validates the `enum` keyword.
//!
//! WHY: The `enum` keyword restricts the instance to one of a fixed set of values.

use alloc::boxed::Box;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct EnumValidator {
    options: Vec<Value>,
    schema_path: Location,
}

impl EnumValidator {
    pub fn new(options: Vec<Value>, schema_path: Location) -> Self {
        Self {
            options,
            schema_path,
        }
    }
}

impl Validate for EnumValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        self.options.iter().any(|opt| values_equal(instance, opt))
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
                    .build(ValidationErrorKind::InvalidEnum {
                        options: self.options.clone(),
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

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
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
    fn matches() {
        let v = EnumValidator::new(vec![json!(1), json!("a"), json!(null)], Location::new());
        assert!(v.is_valid(&json!(1), &mut ctx()));
        assert!(v.is_valid(&json!("a"), &mut ctx()));
        assert!(v.is_valid(&json!(null), &mut ctx()));
    }

    #[test]
    fn no_match() {
        let v = EnumValidator::new(vec![json!(1), json!(2)], Location::new());
        assert!(!v.is_valid(&json!(3), &mut ctx()));
    }

    #[test]
    fn number_coercion() {
        let v = EnumValidator::new(vec![json!(1)], Location::new());
        assert!(v.is_valid(&json!(1.0), &mut ctx()));
    }
}
