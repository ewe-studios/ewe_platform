//! Stub: Legacy `dependencies` keyword (Draft 4/6/7).
//!
//! In Draft 4/6/7, `dependencies` maps property names to either:
//! - An array of required property names (dependentRequired)
//! - A sub-schema to validate (dependentSchemas)
//!
//! In Draft 2019-09+, this is split into `dependentRequired` and `dependentSchemas`.

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError};
use crate::paths::LazyLocation;

use super::{Validate, ValidationContext};

#[allow(dead_code)]
pub struct DependenciesValidator;

impl Validate for DependenciesValidator {
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
