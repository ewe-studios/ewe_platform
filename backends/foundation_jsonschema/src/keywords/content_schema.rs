//! `contentSchema` — validates decoded content against a JSON Schema.
//!
//! WHY: JSON Schema 2019-09+ allows `contentSchema` to specify a schema that
//! the decoded content must conform to. This is used alongside `contentEncoding`
//! and `contentMediaType` (e.g., base64-encoded JSON that must match a schema).
//!
//! The validation flow is:
//! 1. If `contentEncoding` is present, decode the string to bytes
//! 2. If `contentMediaType` is `application/json`, parse bytes as JSON
//! 3. Validate the resulting value against the `contentSchema`

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::node::SchemaNode;
use crate::paths::{LazyLocation, Location};

use super::content::ContentEncoding;
use super::{Validate, ValidationContext};

/// Validates decoded content against a schema.
///
/// When used standalone (no encoding/media type), validates the string
/// directly by parsing as JSON. When combined with encoding, decodes first.
pub struct ContentSchemaValidator {
    schema: SchemaNode,
    encoding: Option<ContentEncoding>,
    media_type: Option<String>,
    schema_path: Location,
}

impl ContentSchemaValidator {
    /// Create a new contentSchema validator.
    ///
    /// `encoding`: optional encoding to decode from (None = treat string as-is).
    /// `media_type`: optional media type (None or "application/json" = parse as JSON).
    #[must_use]
    pub fn new(
        schema: SchemaNode,
        encoding: Option<ContentEncoding>,
        media_type: Option<String>,
        schema_path: Location,
    ) -> Self {
        Self {
            schema,
            encoding,
            media_type,
            schema_path,
        }
    }
}

impl Validate for ContentSchemaValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        let Value::String(s) = instance else {
            return true;
        };
        let Some(content_value) = self.decode_and_parse(s) else {
            return false;
        };
        self.schema.is_valid(&content_value, ctx)
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        let Value::String(s) = instance else {
            return Ok(());
        };
        let Some(content_value) = self.decode_and_parse(s) else {
            return Err(ValidationErrorBuilder::new(
                instance_path.materialize(),
                self.schema_path.clone(),
            )
            .build(ValidationErrorKind::ContentSchema));
        };
        self.schema
            .validate(&content_value, instance_path, ctx)
            .map_err(|_| {
                ValidationErrorBuilder::new(instance_path.materialize(), self.schema_path.clone())
                    .build(ValidationErrorKind::ContentSchema)
            })
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        let Value::String(s) = instance else {
            return Box::new(core::iter::empty());
        };
        let Some(content_value) = self.decode_and_parse(s) else {
            let err =
                ValidationErrorBuilder::new(instance_path.materialize(), self.schema_path.clone())
                    .build(ValidationErrorKind::ContentSchema);
            return Box::new(core::iter::once(err));
        };
        self.schema.iter_errors(&content_value, instance_path, ctx)
    }
}

impl ContentSchemaValidator {
    /// Decode (if encoding present) and parse (if JSON media type) the string.
    fn decode_and_parse(&self, s: &str) -> Option<Value> {
        // Step 1: Decode if encoding is specified
        let bytes: Vec<u8> = match &self.encoding {
            Some(enc) => enc.decode(s)?,
            None => s.as_bytes().to_vec(),
        };

        // Step 2: Parse as JSON if media type is application/json
        // or if no encoding was specified (string is likely JSON)
        if self.media_type.as_deref() == Some("application/json") || self.encoding.is_none() {
            let decoded_str = core::str::from_utf8(&bytes).ok()?;
            serde_json::from_str::<Value>(decoded_str).ok()
        } else {
            // No JSON parsing needed — treat decoded bytes as string value
            let decoded_str = core::str::from_utf8(&bytes).ok()?;
            Some(Value::String(decoded_str.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keywords::type_::TypeValidator;
    use crate::paths::{LazyLocation, Location};
    use crate::types::JsonType;
    use crate::types::JsonTypeSet;
    use serde_json::json;

    fn ctx() -> ValidationContext {
        ValidationContext::new()
    }

    fn string_type_schema() -> SchemaNode {
        let mut types = JsonTypeSet::new();
        types.insert(JsonType::String);
        SchemaNode::Validators {
            validators: vec![Box::new(TypeValidator::new(types, Location::new()))],
            schema_path: Location::new(),
        }
    }

    fn object_type_schema() -> SchemaNode {
        let mut types = JsonTypeSet::new();
        types.insert(JsonType::Object);
        SchemaNode::Validators {
            validators: vec![Box::new(TypeValidator::new(types, Location::new()))],
            schema_path: Location::new(),
        }
    }

    #[test]
    fn content_schema_standalone_valid_json() {
        let v = ContentSchemaValidator::new(object_type_schema(), None, None, Location::new());
        assert!(v.is_valid(&json!({"name": "test"}), &mut ctx()));
    }

    #[test]
    fn content_schema_standalone_invalid_json() {
        let v = ContentSchemaValidator::new(object_type_schema(), None, None, Location::new());
        assert!(!v.is_valid(&json!("not json at all"), &mut ctx()));
    }

    #[test]
    fn content_schema_standalone_json_wrong_type() {
        let v = ContentSchemaValidator::new(object_type_schema(), None, None, Location::new());
        assert!(!v.is_valid(&json!("[1,2,3]"), &mut ctx()));
    }

    #[test]
    fn content_schema_base64_encoded_json() {
        let encoded = "eyJhbnN3ZXIiOjQyfQ==";
        let v = ContentSchemaValidator::new(
            object_type_schema(),
            Some(ContentEncoding::Base64),
            Some("application/json".into()),
            Location::new(),
        );
        assert!(v.is_valid(&json!(encoded), &mut ctx()));
    }

    #[test]
    fn content_schema_base64_wrong_schema() {
        // base64 of "hello" — decodes to string, not valid JSON object
        let encoded = "aGVsbG8=";
        let v = ContentSchemaValidator::new(
            object_type_schema(),
            Some(ContentEncoding::Base64),
            Some("application/json".into()),
            Location::new(),
        );
        assert!(!v.is_valid(&json!(encoded), &mut ctx()));
    }

    #[test]
    fn content_schema_base64_bad_encoding() {
        let v = ContentSchemaValidator::new(
            object_type_schema(),
            Some(ContentEncoding::Base64),
            None,
            Location::new(),
        );
        assert!(!v.is_valid(&json!("not!!base64"), &mut ctx()));
    }

    #[test]
    fn content_schema_non_string_valid() {
        let v = ContentSchemaValidator::new(object_type_schema(), None, None, Location::new());
        assert!(v.is_valid(&json!(42), &mut ctx()));
        assert!(v.is_valid(&json!([1, 2]), &mut ctx()));
    }

    #[test]
    fn content_schema_iter_errors() {
        let v = ContentSchemaValidator::new(object_type_schema(), None, None, Location::new());
        let errors: Vec<_> = v
            .iter_errors(&json!("[1,2]"), &LazyLocation::new(), &mut ctx())
            .collect();
        assert!(!errors.is_empty());
    }
}
