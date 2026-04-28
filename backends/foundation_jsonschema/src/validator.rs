//! Stub: Validator — will be implemented in Feature 3.

use crate::error::{ErrorIterator, ValidationError};
use crate::draft::Draft;
use serde_json::Value;

pub struct Validator;

impl Validator {
    pub fn is_valid(&self, _instance: &Value) -> bool {
        unimplemented!()
    }

    pub fn validate(&self, _instance: &Value) -> Result<(), ValidationError> {
        unimplemented!()
    }

    pub fn iter_errors(&self, _instance: &Value) -> ErrorIterator {
        unimplemented!()
    }

    pub fn draft(&self) -> Draft {
        unimplemented!()
    }
}
