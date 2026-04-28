//! Stub: `contentEncoding` / `contentMediaType` / `contentSchema`.
//!
//! Content validation is a thin layer — actual encoding checks are done
//! during Feature 7 format validation.

use alloc::boxed::Box;
use alloc::string::String;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

pub struct ContentEncodingValidator {
    encoding: String,
    schema_path: Location,
}

impl ContentEncodingValidator {
    pub fn new(encoding: String, schema_path: Location) -> Self {
        Self { encoding, schema_path }
    }
}

impl Validate for ContentEncodingValidator {
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
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        if self.is_valid(instance, ctx) {
            Box::new(core::iter::empty())
        } else {
            Box::new(core::iter::once(
                ValidationErrorBuilder::new(
                    instance_path.materialize(),
                    self.schema_path.clone(),
                )
                .build(ValidationErrorKind::ContentEncoding {
                    encoding: self.encoding.clone(),
                }),
            ))
        }
    }
}

pub struct ContentMediaTypeValidator {
    media_type: String,
    schema_path: Location,
}

impl ContentMediaTypeValidator {
    pub fn new(media_type: String, schema_path: Location) -> Self {
        Self { media_type, schema_path }
    }
}

impl Validate for ContentMediaTypeValidator {
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
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        if self.is_valid(instance, ctx) {
            Box::new(core::iter::empty())
        } else {
            Box::new(core::iter::once(
                ValidationErrorBuilder::new(
                    instance_path.materialize(),
                    self.schema_path.clone(),
                )
                .build(ValidationErrorKind::ContentMediaType {
                    media_type: self.media_type.clone(),
                }),
            ))
        }
    }
}
