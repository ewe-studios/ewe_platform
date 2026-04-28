//! Validates `maximum` — numeric value must be <= (or <) a limit.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct MaximumValidator {
    limit: f64,
    exclusive: bool,
    schema_path: Location,
}

impl MaximumValidator {
    pub fn new(limit: f64, exclusive: bool, schema_path: Location) -> Self {
        Self { limit, exclusive, schema_path }
    }
}

impl Validate for MaximumValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if let Value::Number(n) = instance {
            if let Some(v) = n.as_f64() {
                return if self.exclusive { v < self.limit } else { v <= self.limit };
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
                let failed = if self.exclusive {
                    v >= self.limit
                } else {
                    v > self.limit
                };
                if failed {
                    return Err(ValidationErrorBuilder::new(
                        instance_path.materialize(),
                        self.schema_path.clone(),
                    )
                    .build(ValidationErrorKind::Maximum {
                        limit: self.limit,
                        actual: v,
                        exclusive: self.exclusive,
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
        ValidationContext
    }

    #[test]
    fn inclusive_equal() {
        let v = MaximumValidator::new(5.0, false, Location::new());
        assert!(v.is_valid(&json!(5), &mut ctx()));
    }

    #[test]
    fn exclusive_equal() {
        let v = MaximumValidator::new(5.0, true, Location::new());
        assert!(!v.is_valid(&json!(5), &mut ctx()));
    }

    #[test]
    fn non_number_always_valid() {
        let v = MaximumValidator::new(0.0, false, Location::new());
        assert!(v.is_valid(&json!("hello"), &mut ctx()));
    }
}
