//! Validates `propertyNames` — all property names must match a schema.

use alloc::boxed::Box;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct PropertyNamesValidator {
    sub_validators: Vec<Box<dyn Validate>>,
    schema_path: Location,
}

impl PropertyNamesValidator {
    pub fn new(sub_validators: Vec<Box<dyn Validate>>, schema_path: Location) -> Self {
        Self { sub_validators, schema_path }
    }
}

impl Validate for PropertyNamesValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        if let Value::Object(obj) = instance {
            obj.keys().all(|key| {
                let name = Value::String(key.clone());
                self.sub_validators.iter().all(|sv| sv.is_valid(&name, ctx))
            })
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
            for key in obj.keys() {
                let name = Value::String(key.clone());
                for sv in &self.sub_validators {
                    sv.validate(
                        &name,
                        &instance_path.push_property(key),
                        ctx,
                    )?;
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
        if let Value::Object(obj) = instance {
            let mut errors: Vec<crate::error::ValidationError> = Vec::new();
            for key in obj.keys() {
                let name = Value::String(key.clone());
                for sv in &self.sub_validators {
                    for e in sv.iter_errors(&name, &instance_path.push_property(key), ctx) {
                        errors.push(e);
                    }
                }
            }
            Box::new(errors.into_iter())
        } else {
            Box::new(core::iter::empty())
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
    fn all_valid_names() {
        let v = PropertyNamesValidator::new(vec![], Location::new());
        assert!(v.is_valid(&json!({"a": 1}), &mut ctx()));
    }
}
