//! Validates `minProperties` — object must have at least N properties.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct MinPropertiesValidator {
    min: u64,
    schema_path: Location,
}

impl MinPropertiesValidator {
    pub fn new(min: u64, schema_path: Location) -> Self {
        Self { min, schema_path }
    }
}

impl Validate for MinPropertiesValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if let Value::Object(obj) = instance {
            obj.len() as u64 >= self.min
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
        if let Value::Object(obj) = instance {
            let actual = obj.len() as u64;
            if actual < self.min {
                return Err(ValidationErrorBuilder::new(
                    instance_path.materialize(),
                    self.schema_path.clone(),
                )
                .build(ValidationErrorKind::MinProperties { min: self.min, actual }));
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
    fn enough_props() {
        let v = MinPropertiesValidator::new(2, Location::new());
        assert!(v.is_valid(&json!({"a": 1, "b": 2}), &mut ctx()));
    }

    #[test]
    fn too_few() {
        let v = MinPropertiesValidator::new(2, Location::new());
        assert!(!v.is_valid(&json!({"a": 1}), &mut ctx()));
    }

    #[test]
    fn non_object_always_valid() {
        let v = MinPropertiesValidator::new(100, Location::new());
        assert!(v.is_valid(&json!(42), &mut ctx()));
    }
}
