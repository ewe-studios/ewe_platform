//! `additionalProperties` — validates properties not covered by `properties` or `patternProperties`.

use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use regex::Regex;
use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::node::SchemaNode;
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

/// How additional properties should be validated.
pub enum AdditionalSchema {
    /// `additionalProperties: false` — reject all additional properties.
    False,
    /// `additionalProperties: { ... }` — validate against a sub-schema.
    Schema(SchemaNode),
}

/// Validates additional properties against a schema or rejects them.
pub struct AdditionalPropertiesValidator {
    schema: Option<AdditionalSchema>,
    /// Property names defined by `properties` keyword.
    defined_properties: BTreeSet<String>,
    /// Regex patterns from `patternProperties` keyword.
    pattern_regexes: Vec<(Regex, String)>,
}

impl AdditionalPropertiesValidator {
    /// Create a new validator.
    ///
    /// `schema`: None means additionalProperties is absent (allow all).
    ///           Some(False) means reject all additional props.
    ///           Some(Schema) means validate against the schema.
    #[must_use]
    pub fn new(
        schema: Option<AdditionalSchema>,
        defined_properties: BTreeSet<String>,
        pattern_regexes: Vec<(Regex, String)>,
    ) -> Self {
        Self {
            schema,
            defined_properties,
            pattern_regexes,
        }
    }

    fn is_additional_property(&self, name: &str) -> bool {
        if self.defined_properties.contains(name) {
            return false;
        }
        for (regex, _) in &self.pattern_regexes {
            if regex.is_match(name) {
                return false;
            }
        }
        true
    }
}

impl Validate for AdditionalPropertiesValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        let Some(ref schema) = self.schema else {
            return true;
        };
        if let Value::Object(obj) = instance {
            for name in obj.keys() {
                if self.is_additional_property(name) {
                    let value = &obj[name];
                    match schema {
                        AdditionalSchema::False => return false,
                        AdditionalSchema::Schema(s) => {
                            if !s.is_valid(value, ctx) {
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
        let Some(ref schema) = self.schema else {
            return Ok(());
        };
        if let Value::Object(obj) = instance {
            for name in obj.keys() {
                if self.is_additional_property(name) {
                    let value = &obj[name];
                    match schema {
                        AdditionalSchema::False => {
                            return Err(ValidationErrorBuilder::new(
                                instance_path.push_property(name).materialize(),
                                Location::new(),
                            )
                            .build(
                                ValidationErrorKind::AdditionalProperties {
                                    unexpected: alloc::vec![name.clone()],
                                },
                            ));
                        }
                        AdditionalSchema::Schema(s) => {
                            let child_path = instance_path.push_property(name);
                            s.validate(value, &child_path, ctx)?;
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
        let Some(ref schema) = self.schema else {
            return Box::new(errors.into_iter());
        };
        if let Value::Object(obj) = instance {
            for name in obj.keys() {
                if self.is_additional_property(name) {
                    let value = &obj[name];
                    match schema {
                        AdditionalSchema::False => {
                            errors.push(
                                ValidationErrorBuilder::new(
                                    instance_path.push_property(name).materialize(),
                                    Location::new(),
                                )
                                .build(
                                    ValidationErrorKind::AdditionalProperties {
                                        unexpected: alloc::vec![name.clone()],
                                    },
                                ),
                            );
                        }
                        AdditionalSchema::Schema(s) => {
                            let child_path = instance_path.push_property(name);
                            for e in s.iter_errors(value, &child_path, ctx) {
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
