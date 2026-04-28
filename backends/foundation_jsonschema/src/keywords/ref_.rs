//! Stub: `$ref` — delegates to the referenced schema.
//!
//! Requires SchemaNode (Feature 3) for compiled sub-schema resolution.

use alloc::boxed::Box;
use alloc::string::String;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct RefValidator {
    reference: String,
}

impl RefValidator {
    pub fn new(reference: String) -> Self {
        Self { reference }
    }
}

impl Validate for RefValidator {
    fn is_valid(&self, _instance: &Value, _ctx: &mut ValidationContext) -> bool {
        true
    }

    fn validate(
        &self,
        _instance: &Value,
        _instance_path: &LazyLocation<'_>,
        _ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        Ok(())
    }

    fn iter_errors(
        &self,
        _instance: &Value,
        _instance_path: &LazyLocation<'_>,
        _ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        Box::new(core::iter::empty())
    }
}

pub struct DynamicRefValidator;

impl Validate for DynamicRefValidator {
    fn is_valid(&self, _instance: &Value, _ctx: &mut ValidationContext) -> bool {
        true
    }

    fn validate(
        &self,
        _instance: &Value,
        _instance_path: &LazyLocation<'_>,
        _ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        Ok(())
    }

    fn iter_errors(
        &self,
        _instance: &Value,
        _instance_path: &LazyLocation<'_>,
        _ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        Box::new(core::iter::empty())
    }
}

pub struct RecursiveRefValidator;

impl Validate for RecursiveRefValidator {
    fn is_valid(&self, _instance: &Value, _ctx: &mut ValidationContext) -> bool {
        true
    }

    fn validate(
        &self,
        _instance: &Value,
        _instance_path: &LazyLocation<'_>,
        _ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        Ok(())
    }

    fn iter_errors(
        &self,
        _instance: &Value,
        _instance_path: &LazyLocation<'_>,
        _ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        Box::new(core::iter::empty())
    }
}
