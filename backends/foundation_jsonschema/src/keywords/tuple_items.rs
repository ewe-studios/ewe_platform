//! `items` as array (Draft 4/6/7/2019-09 tuple validation).
//!
//! When `items` is an array, each position in the array defines a schema
//! for the corresponding position in the instance array (tuple validation).
//! Items beyond the tuple length are validated by `additionalItems`.

use alloc::boxed::Box;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorKind};
use crate::node::SchemaNode;
use crate::paths::LazyLocation;
use foundation_errstacks::IntoErrorTrace;

use super::{Validate, ValidationContext};

/// How to handle items beyond the tuple length.
pub enum AdditionalItemsPolicy {
    /// Allow any additional items (additionalItems: true or absent).
    AllowAny,
    /// Reject any additional items (additionalItems: false).
    RejectAll,
    /// Validate additional items against this schema.
    Validate(SchemaNode),
}

/// Validates array items positionally (tuple validation).
///
/// Items up to `schemas.len()` are validated against their positional schema.
/// Items beyond the tuple length are handled according to `additional`.
pub struct TupleItemsValidator {
    /// Tuple schemas — one per position.
    schemas: Vec<SchemaNode>,
    /// How to handle items beyond the tuple length.
    additional: AdditionalItemsPolicy,
}

impl TupleItemsValidator {
    /// Create a new tuple validator.
    ///
    /// `schemas`: positional schemas for each tuple position.
    /// `additional`: policy for items beyond the tuple length.
    #[must_use]
    pub fn new(schemas: Vec<SchemaNode>, additional: AdditionalItemsPolicy) -> Self {
        Self {
            schemas,
            additional,
        }
    }
}

impl Validate for TupleItemsValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        if let Value::Array(arr) = instance {
            for (i, item) in arr.iter().enumerate() {
                let schema = if i < self.schemas.len() {
                    &self.schemas[i]
                } else {
                    match &self.additional {
                        AdditionalItemsPolicy::AllowAny => return true,
                        AdditionalItemsPolicy::RejectAll => return false,
                        AdditionalItemsPolicy::Validate(s) => s,
                    }
                };
                if !schema.is_valid(item, ctx) {
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
            for (i, item) in arr.iter().enumerate() {
                let child_path = instance_path.push_index(i);
                let schema = if i < self.schemas.len() {
                    &self.schemas[i]
                } else {
                    match &self.additional {
                        AdditionalItemsPolicy::AllowAny => continue,
                        AdditionalItemsPolicy::RejectAll => {
                            return Err(ValidationErrorKind::AdditionalItems {
                                limit: self.schemas.len(),
                            }
                            .into_error_trace());
                        }
                        AdditionalItemsPolicy::Validate(s) => s,
                    }
                };
                schema.validate(item, &child_path, ctx)?;
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
            for (i, item) in arr.iter().enumerate() {
                let child_path = instance_path.push_index(i);
                let schema = if i < self.schemas.len() {
                    &self.schemas[i]
                } else {
                    match &self.additional {
                        AdditionalItemsPolicy::AllowAny => continue,
                        AdditionalItemsPolicy::RejectAll => {
                            errors.push(
                                ValidationErrorKind::AdditionalItems {
                                    limit: self.schemas.len(),
                                }
                                .into_error_trace(),
                            );
                            continue;
                        }
                        AdditionalItemsPolicy::Validate(s) => s,
                    }
                };
                for e in schema.iter_errors(item, &child_path, ctx) {
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
    use crate::node::SchemaNode;
    use crate::paths::{LazyLocation, Location};
    use serde_json::json;

    fn ctx() -> ValidationContext {
        ValidationContext::new()
    }

    #[test]
    fn tuple_exact_length_valid() {
        let v = TupleItemsValidator::new(
            vec![SchemaNode::AlwaysValid],
            AdditionalItemsPolicy::RejectAll,
        );
        assert!(v.is_valid(&json!([1]), &mut ctx()));
    }

    #[test]
    fn tuple_exact_length_too_many_items() {
        let v = TupleItemsValidator::new(
            vec![SchemaNode::AlwaysValid],
            AdditionalItemsPolicy::RejectAll,
        );
        assert!(!v.is_valid(&json!([1, 2]), &mut ctx()));
    }

    #[test]
    fn tuple_fewer_items_ok() {
        let v = TupleItemsValidator::new(
            vec![SchemaNode::AlwaysValid, SchemaNode::AlwaysValid],
            AdditionalItemsPolicy::RejectAll,
        );
        assert!(v.is_valid(&json!([1]), &mut ctx()));
    }

    #[test]
    fn tuple_additional_items_allow_any() {
        let v = TupleItemsValidator::new(
            vec![SchemaNode::AlwaysValid],
            AdditionalItemsPolicy::AllowAny,
        );
        assert!(v.is_valid(&json!([1, "anything", true, null]), &mut ctx()));
    }

    #[test]
    fn tuple_additional_items_validate_schema() {
        let v = TupleItemsValidator::new(
            vec![SchemaNode::AlwaysValid],
            AdditionalItemsPolicy::Validate(SchemaNode::AlwaysValid),
        );
        assert!(v.is_valid(&json!([1, 2, 3]), &mut ctx()));
    }

    #[test]
    fn tuple_empty_with_reject_all() {
        let v = TupleItemsValidator::new(vec![], AdditionalItemsPolicy::RejectAll);
        assert!(v.is_valid(&json!([]), &mut ctx()));
        assert!(!v.is_valid(&json!([1]), &mut ctx()));
    }

    #[test]
    fn tuple_non_array_instance() {
        let v = TupleItemsValidator::new(
            vec![SchemaNode::AlwaysValid],
            AdditionalItemsPolicy::RejectAll,
        );
        assert!(v.is_valid(&json!("not an array"), &mut ctx()));
        assert!(v.is_valid(&json!({"key": "value"}), &mut ctx()));
    }

    #[test]
    fn tuple_collects_errors_from_multiple_positions() {
        let v = TupleItemsValidator::new(
            vec![
                SchemaNode::AlwaysInvalid {
                    schema_path: Location::new(),
                },
                SchemaNode::AlwaysInvalid {
                    schema_path: Location::new(),
                },
            ],
            AdditionalItemsPolicy::RejectAll,
        );
        let errors: Vec<_> = v
            .iter_errors(&json!(["a", "b"]), &LazyLocation::new(), &mut ctx())
            .collect();
        assert_eq!(errors.len(), 2);
    }
}
