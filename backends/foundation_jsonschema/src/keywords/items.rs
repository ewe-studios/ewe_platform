//! `items` — validates all array items against a sub-schema.
//!
//! For Draft 4/6/7, `items` can be a single schema (applies to all items)
//! or an array of schemas (tuple validation with `additionalItems`).
//! For Draft 2020-12, `items` always applies to all items, but when combined
//! with `prefixItems`, it only validates items beyond the prefix count.

use alloc::boxed::Box;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError};
use crate::node::SchemaNode;
use crate::paths::LazyLocation;

use super::{Validate, ValidationContext};

/// Validates all array items against a sub-schema.
pub struct ItemsValidator {
    schema: SchemaNode,
    /// Number of initial items to skip (used with prefixItems in Draft 2020-12).
    skip_first: usize,
}

impl ItemsValidator {
    /// Create with a pre-compiled item schema, applying to all items.
    #[must_use]
    pub fn new(schema: SchemaNode) -> Self {
        Self {
            schema,
            skip_first: 0,
        }
    }

    /// Create with a pre-compiled item schema, skipping the first N items.
    #[must_use]
    pub fn with_offset(schema: SchemaNode, skip_first: usize) -> Self {
        Self { schema, skip_first }
    }
}

impl Validate for ItemsValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        if let Value::Array(arr) = instance {
            for item in arr.iter().skip(self.skip_first) {
                if !self.schema.is_valid(item, ctx) {
                    return false;
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
        if let Value::Array(arr) = instance {
            for (i, item) in arr.iter().enumerate().skip(self.skip_first) {
                let child_path = instance_path.push_index(i);
                self.schema.validate(item, &child_path, ctx)?;
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
        if let Value::Array(arr) = instance {
            for (i, item) in arr.iter().enumerate().skip(self.skip_first) {
                let child_path = instance_path.push_index(i);
                for e in self.schema.iter_errors(item, &child_path, ctx) {
                    errors.push(e);
                }
            }
        }
        Box::new(errors.into_iter())
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

    #[test]
    fn items_all_valid() {
        let v = ItemsValidator::new(SchemaNode::AlwaysValid);
        assert!(v.is_valid(&json!([1, "two", true]), &mut ctx()));
    }

    #[test]
    fn items_type_check() {
        let v = ItemsValidator::new(string_type_schema());
        assert!(v.is_valid(&json!(["a", "b", "c"]), &mut ctx()));
        assert!(!v.is_valid(&json!(["a", 42, "c"]), &mut ctx()));
    }

    #[test]
    fn items_with_offset_skips_prefix() {
        // Offset 2: first 2 items are skipped, remaining validated
        let v = ItemsValidator::with_offset(string_type_schema(), 2);
        // First 2 items can be anything, rest must be strings
        assert!(v.is_valid(&json!([1, 2, "ok", "fine"]), &mut ctx()));
        assert!(!v.is_valid(&json!([1, 2, 42, "fine"]), &mut ctx()));
    }

    #[test]
    fn items_offset_beyond_array_length() {
        let v = ItemsValidator::with_offset(string_type_schema(), 10);
        assert!(v.is_valid(&json!([1, 2, 3]), &mut ctx()));
    }

    #[test]
    fn items_non_array_instance() {
        let v = ItemsValidator::new(string_type_schema());
        assert!(v.is_valid(&json!("not an array"), &mut ctx()));
    }

    #[test]
    fn items_empty_array() {
        let v = ItemsValidator::new(string_type_schema());
        assert!(v.is_valid(&json!([]), &mut ctx()));
    }

    #[test]
    fn items_collects_errors() {
        let v = ItemsValidator::new(string_type_schema());
        let errors: Vec<_> = v
            .iter_errors(&json!([1, 2, 3]), &LazyLocation::new(), &mut ctx())
            .collect();
        assert_eq!(errors.len(), 3);
    }
}
