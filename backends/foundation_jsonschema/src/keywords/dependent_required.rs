//! `dependentRequired` — property presence triggers required others.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct DependentRequiredValidator {
    map: BTreeMap<String, Vec<String>>,
    schema_path: Location,
}

impl DependentRequiredValidator {
    pub fn new(map: BTreeMap<String, Vec<String>>, schema_path: Location) -> Self {
        Self { map, schema_path }
    }
}

impl Validate for DependentRequiredValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if let Value::Object(obj) = instance {
            for (prop, required) in &self.map {
                if obj.contains_key(prop.as_str()) {
                    for req in required {
                        if !obj.contains_key(req.as_str()) {
                            return false;
                        }
                    }
                }
            }
            true
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
            for (prop, required) in &self.map {
                if obj.contains_key(prop.as_str()) {
                    let mut missing = Vec::new();
                    for req in required {
                        if !obj.contains_key(req.as_str()) {
                            missing.push(req.clone());
                        }
                    }
                    if !missing.is_empty() {
                        return Err(ValidationErrorBuilder::new(
                            instance_path.materialize(),
                            self.schema_path.clone(),
                        )
                        .build(ValidationErrorKind::DependentRequired {
                            property: prop.clone(),
                            missing,
                        }));
                    }
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
    fn dependency_satisfied() {
        let mut map = BTreeMap::new();
        map.insert("a".into(), vec!["b".into()]);
        let v = DependentRequiredValidator::new(map, Location::new());
        assert!(v.is_valid(&json!({"a": 1, "b": 2}), &mut ctx()));
    }

    #[test]
    fn dependency_missing() {
        let mut map = BTreeMap::new();
        map.insert("a".into(), vec!["b".into()]);
        let v = DependentRequiredValidator::new(map, Location::new());
        assert!(!v.is_valid(&json!({"a": 1}), &mut ctx()));
    }

    #[test]
    fn no_trigger_always_valid() {
        let mut map = BTreeMap::new();
        map.insert("a".into(), vec!["b".into()]);
        let v = DependentRequiredValidator::new(map, Location::new());
        assert!(v.is_valid(&json!({"c": 3}), &mut ctx()));
    }
}
