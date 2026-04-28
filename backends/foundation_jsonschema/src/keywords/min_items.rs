//! Validates `minItems` — array must have at least N elements.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct MinItemsValidator {
    min: u64,
    schema_path: Location,
}

impl MinItemsValidator {
    pub fn new(min: u64, schema_path: Location) -> Self {
        Self { min, schema_path }
    }
}

impl Validate for MinItemsValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if let Value::Array(arr) = instance {
            arr.len() as u64 >= self.min
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
        if let Value::Array(arr) = instance {
            let actual = arr.len() as u64;
            if actual < self.min {
                return Err(ValidationErrorBuilder::new(
                    instance_path.materialize(),
                    self.schema_path.clone(),
                )
                .build(ValidationErrorKind::MinItems { min: self.min, actual }));
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
    fn enough_items() {
        let v = MinItemsValidator::new(2, Location::new());
        assert!(v.is_valid(&json!([1, 2, 3]), &mut ctx()));
    }

    #[test]
    fn too_few_items() {
        let v = MinItemsValidator::new(3, Location::new());
        assert!(!v.is_valid(&json!([1, 2]), &mut ctx()));
    }

    #[test]
    fn non_array_always_valid() {
        let v = MinItemsValidator::new(100, Location::new());
        assert!(v.is_valid(&json!("hello"), &mut ctx()));
    }
}
