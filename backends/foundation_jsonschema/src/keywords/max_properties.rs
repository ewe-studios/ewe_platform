//! Validates `maxProperties` — object must have at most N properties.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct MaxPropertiesValidator {
    max: u64,
    schema_path: Location,
}

impl MaxPropertiesValidator {
    pub fn new(max: u64, schema_path: Location) -> Self {
        Self { max, schema_path }
    }
}

impl Validate for MaxPropertiesValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if let Value::Object(obj) = instance {
            obj.len() as u64 <= self.max
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
        if let Value::Object(obj) = instance {
            let actual = obj.len() as u64;
            if actual > self.max {
                return Err(ValidationErrorBuilder::new(
                    instance_path.materialize(),
                    self.schema_path.clone(),
                )
                .build(ValidationErrorKind::MaxProperties {
                    max: self.max,
                    actual,
                }));
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
        let v = MaxPropertiesValidator::new(2, Location::new());
        assert!(v.is_valid(&json!({"a": 1, "b": 2}), &mut ctx()));
    }

    #[test]
    fn too_many() {
        let v = MaxPropertiesValidator::new(1, Location::new());
        assert!(!v.is_valid(&json!({"a": 1, "b": 2}), &mut ctx()));
    }

    #[test]
    fn non_object_always_valid() {
        let v = MaxPropertiesValidator::new(0, Location::new());
        assert!(v.is_valid(&json!(42), &mut ctx()));
    }
}
