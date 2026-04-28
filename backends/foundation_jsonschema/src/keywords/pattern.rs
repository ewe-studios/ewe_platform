//! Validates `pattern` — string must match a regex.

use alloc::boxed::Box;

use regex::Regex;
use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct PatternValidator {
    regex: Regex,
    raw_pattern: alloc::string::String,
    schema_path: Location,
}

impl PatternValidator {
    pub fn new(pattern: alloc::string::String, schema_path: Location) -> Self {
        let regex = Regex::new(&pattern).expect("pattern must be a valid regex");
        Self { regex, raw_pattern: pattern, schema_path }
    }
}

impl Validate for PatternValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        if let Value::String(s) = instance {
            self.regex.is_match(s)
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
        if self.is_valid(instance, ctx) {
            Ok(())
        } else {
            Err(ValidationErrorBuilder::new(
                instance_path.materialize(),
                self.schema_path.clone(),
            )
            .build(ValidationErrorKind::Pattern {
                pattern: self.raw_pattern.clone(),
            }))
        }
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
    fn matches_pattern() {
        let v = PatternValidator::new(r"^\d+$".into(), Location::new());
        assert!(v.is_valid(&json!("12345"), &mut ctx()));
    }

    #[test]
    fn no_match_pattern() {
        let v = PatternValidator::new(r"^\d+$".into(), Location::new());
        assert!(!v.is_valid(&json!("abc"), &mut ctx()));
    }

    #[test]
    fn non_string_always_valid() {
        let v = PatternValidator::new(r"^\d+$".into(), Location::new());
        assert!(v.is_valid(&json!(42), &mut ctx()));
    }
}
