//! Stub: `additionalProperties` — handles extra properties.
//!
//! Requires SchemaNode (Feature 3).

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError};
use crate::paths::LazyLocation;

use super::{Validate, ValidationContext};

pub struct AdditionalPropertiesValidator;

impl Validate for AdditionalPropertiesValidator {
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
