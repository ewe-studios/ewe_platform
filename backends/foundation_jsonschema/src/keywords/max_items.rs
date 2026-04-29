//! Validates `maxItems` — array must have at most N elements.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct MaxItemsValidator {
    max: u64,
    schema_path: Location,
}

impl MaxItemsValidator {
    pub fn new(max: u64, schema_path: Location) -> Self {
        Self { max, schema_path }
    }
}

impl Validate for MaxItemsValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if let Value::Array(arr) = instance {
            arr.len() as u64 <= self.max
        } else {
            true
        }
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        _ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        if let Value::Array(arr) = instance {
            let actual = arr.len() as u64;
            if actual > self.max {
                return Err(ValidationErrorBuilder::new(
                    instance_path.materialize(),
                    self.schema_path.clone(),
                )
                .build(ValidationErrorKind::MaxItems { max: self.max, actual }));
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
    fn exact_count() {
        let v = MaxItemsValidator::new(2, Location::new());
        assert!(v.is_valid(&json!([1, 2]), &mut ctx()));
    }

    #[test]
    fn too_many() {
        let v = MaxItemsValidator::new(1, Location::new());
        assert!(!v.is_valid(&json!([1, 2]), &mut ctx()));
    }

    #[test]
    fn non_array_always_valid() {
        let v = MaxItemsValidator::new(0, Location::new());
        assert!(v.is_valid(&json!("hello"), &mut ctx()));
    }
}
