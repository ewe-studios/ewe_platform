//! Legacy `dependencies` keyword (Draft 4/6/7).
//!
//! WHY: Before Draft 2019-09, `dependencies` combined what are now
//! `dependentRequired` (array of required property names) and
//! `dependentSchemas` (sub-schema validation) into a single keyword.
//! This validator handles both forms for backward compatibility.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::node::SchemaNode;
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

/// Either a list of required properties or a sub-schema.
pub enum Dependency {
    Required(Vec<String>),
    Schema(SchemaNode),
}

/// Validates legacy `dependencies` keyword.
pub struct DependenciesValidator {
    deps: BTreeMap<String, Dependency>,
}

impl DependenciesValidator {
    /// Create with pre-compiled dependencies.
    #[must_use]
    pub fn new(deps: BTreeMap<String, Dependency>) -> Self {
        Self { deps }
    }
}

impl Validate for DependenciesValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        if let Value::Object(obj) = instance {
            for (prop, dep) in &self.deps {
                if obj.contains_key(prop) {
                    match dep {
                        Dependency::Required(missing_props) => {
                            for m in missing_props {
                                if !obj.contains_key(m) {
                                    return false;
                                }
                            }
                        }
                        Dependency::Schema(schema) => {
                            if !schema.is_valid(instance, ctx) {
                                return false;
                            }
                        }
                    }
                }
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
        if let Value::Object(obj) = instance {
            for (prop, dep) in &self.deps {
                if obj.contains_key(prop) {
                    match dep {
                        Dependency::Required(missing_props) => {
                            let missing: Vec<String> = missing_props
                                .iter()
                                .filter(|m| !obj.contains_key(*m))
                                .cloned()
                                .collect();
                            if !missing.is_empty() {
                                return Err(ValidationErrorBuilder::new(
                                    instance_path.materialize(),
                                    Location::new(),
                                )
                                .build(
                                    ValidationErrorKind::DependentRequired {
                                        property: prop.clone(),
                                        missing,
                                    },
                                ));
                            }
                        }
                        Dependency::Schema(schema) => {
                            schema.validate(instance, instance_path, ctx)?;
                        }
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
        let mut errors: Vec<ValidationError> = Vec::new();
        if let Value::Object(obj) = instance {
            for (prop, dep) in &self.deps {
                if obj.contains_key(prop) {
                    match dep {
                        Dependency::Required(missing_props) => {
                            let missing: Vec<String> = missing_props
                                .iter()
                                .filter(|m| !obj.contains_key(*m))
                                .cloned()
                                .collect();
                            if !missing.is_empty() {
                                errors.push(
                                    ValidationErrorBuilder::new(
                                        instance_path.materialize(),
                                        Location::new(),
                                    )
                                    .build(
                                        ValidationErrorKind::DependentRequired {
                                            property: prop.clone(),
                                            missing,
                                        },
                                    ),
                                );
                            }
                        }
                        Dependency::Schema(schema) => {
                            for e in schema.iter_errors(instance, instance_path, ctx) {
                                errors.push(e);
                            }
                        }
                    }
                }
            }
        }
        Box::new(errors.into_iter())
    }
}
