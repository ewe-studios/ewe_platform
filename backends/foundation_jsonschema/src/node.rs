//! Stub: SchemaNode — will be implemented in Feature 3.

use alloc::vec::Vec;

use crate::error::{ErrorIterator, ValidationError};
use crate::paths::LazyLocation;
use crate::keywords::ValidationContext;
use serde_json::Value;

pub enum SchemaNode {
    AlwaysValid,
    AlwaysInvalid { schema_path: crate::paths::Location },
    Validators { validators: Vec<crate::keywords::BoxedValidator>, schema_path: crate::paths::Location },
}

impl SchemaNode {
    pub fn is_valid(&self, _instance: &Value, _ctx: &mut ValidationContext) -> bool {
        unimplemented!()
    }

    pub fn validate(
        &self,
        _instance: &Value,
        _path: &LazyLocation<'_>,
        _ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        unimplemented!()
    }

    pub fn iter_errors(
        &self,
        _instance: &Value,
        _path: &LazyLocation<'_>,
        _ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        unimplemented!()
    }
}
