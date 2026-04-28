//! Validates `exclusiveMaximum` (Draft 6+) — must be strictly less than value.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct ExclusiveMaximumValidator {
    limit: f64,
    schema_path: Location,
}

impl ExclusiveMaximumValidator {
    pub fn new(limit: f64, schema_path: Location) -> Self {
        Self { limit, schema_path }
    }
}

impl Validate for ExclusiveMaximumValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if let Value::Number(n) = instance {
            if let Some(v) = n.as_f64() {
                return v < self.limit;
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
                if v >= self.limit {
                    return Err(ValidationErrorBuilder::new(
                        instance_path.materialize(),
                        self.schema_path.clone(),
                    )
                    .build(ValidationErrorKind::Maximum {
                        limit: self.limit,
                        actual: v,
                        exclusive: true,
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
    fn below_limit() {
        let v = ExclusiveMaximumValidator::new(5.0, Location::new());
        assert!(v.is_valid(&json!(4), &mut ctx()));
    }

    #[test]
    fn at_limit_fails() {
        let v = ExclusiveMaximumValidator::new(5.0, Location::new());
        assert!(!v.is_valid(&json!(5), &mut ctx()));
    }
}
