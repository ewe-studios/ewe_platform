//! Stub: ValidationOptions builder — will be implemented in Feature 3.

use crate::draft::Draft;
use crate::error::ValidationError;
use crate::resolver_trait::NoopResolver;
use crate::validator::Validator;
use serde_json::Value;

pub struct ValidationOptions {
    default_draft: Draft,
    resolver: NoopResolver,
}

impl ValidationOptions {
    pub fn new() -> Self {
        Self {
            default_draft: Draft::DEFAULT,
            resolver: NoopResolver,
        }
    }

    pub fn with_draft(mut self, draft: Draft) -> Self {
        self.default_draft = draft;
        self
    }

    pub fn build(self, _schema: &Value) -> Result<Validator, ValidationError> {
        unimplemented!()
    }
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self::new()
    }
}
