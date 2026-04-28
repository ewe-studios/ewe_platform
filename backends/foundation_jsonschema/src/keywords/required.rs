//! Validates `required` — specified properties must exist.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct RequiredValidator {
    properties: Vec<String>,
    schema_path: Location,
}

impl RequiredValidator {
    pub fn new(properties: Vec<String>, schema_path: Location) -> Self {
        Self { properties, schema_path }
    }
}

impl Validate for RequiredValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if let Value::Object(obj) = instance {
            self.properties.iter().all(|p| obj.contains_key(p))
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
            for prop in &self.properties {
                if !obj.contains_key(prop.as_str()) {
                    return Err(ValidationErrorBuilder::new(
                        instance_path.materialize(),
                        self.schema_path.clone(),
                    )
                    .build(ValidationErrorKind::Required {
                        property: prop.clone(),
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
        if let Value::Object(obj) = instance {
            let mut missing = Vec::new();
            for prop in &self.properties {
                if !obj.contains_key(prop.as_str()) {
                    missing.push(ValidationErrorBuilder::new(
                        instance_path.materialize(),
                        self.schema_path.clone(),
                    )
                    .build(ValidationErrorKind::Required {
                        property: prop.clone(),
                    }));
                }
            }
            Box::new(missing.into_iter())
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
    fn all_present() {
        let v = RequiredValidator::new(vec!["a".into(), "b".into()], Location::new());
        assert!(v.is_valid(&json!({"a": 1, "b": 2}), &mut ctx()));
    }

    #[test]
    fn missing_one() {
        let v = RequiredValidator::new(vec!["a".into(), "b".into()], Location::new());
        assert!(!v.is_valid(&json!({"a": 1}), &mut ctx()));
    }

    #[test]
    fn non_object_always_valid() {
        let v = RequiredValidator::new(vec!["a".into()], Location::new());
        assert!(v.is_valid(&json!(42), &mut ctx()));
    }
}
