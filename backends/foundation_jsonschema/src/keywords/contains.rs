//! `contains` — validates array items against a sub-schema, with optional
//! `minContains` and `maxContains` bounds (Draft 2019-09+).

use alloc::boxed::Box;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::node::SchemaNode;
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

/// Validates that array items matching the sub-schema fall within
/// the `[min_contains, max_contains]` bounds.
///
/// Each matching item is marked as evaluated in the context for
/// `unevaluatedItems` tracking.
pub struct ContainsValidator {
    schema: SchemaNode,
    min_contains: u64,
    max_contains: Option<u64>,
}

impl ContainsValidator {
    /// Create with a pre-compiled sub-schema and no bounds (default: min=1, no max).
    #[must_use]
    pub fn new(schema: SchemaNode) -> Self {
        Self {
            schema,
            min_contains: 1,
            max_contains: None,
        }
    }

    /// Set the minimum number of matching items required.
    #[must_use]
    pub fn with_min(mut self, min: u64) -> Self {
        self.min_contains = min;
        self
    }

    /// Set the maximum number of matching items allowed.
    #[must_use]
    pub fn with_max(mut self, max: u64) -> Self {
        self.max_contains = Some(max);
        self
    }
}

impl Validate for ContainsValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        if let Value::Array(arr) = instance {
            let mut match_count: u64 = 0;
            for (i, item) in arr.iter().enumerate() {
                if self.schema.is_valid(item, ctx) {
                    match_count += 1;
                    ctx.mark_item_evaluated(i);
                }
            }
            return match_count >= self.min_contains
                && self.max_contains.is_none_or(|max| match_count <= max);
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
            let mut match_count: u64 = 0;
            for (i, item) in arr.iter().enumerate() {
                if self.schema.is_valid(item, ctx) {
                    match_count += 1;
                    ctx.mark_item_evaluated(i);
                }
            }
            if match_count < self.min_contains {
                return Err(
                    ValidationErrorBuilder::new(instance_path.materialize(), Location::new())
                        .build(ValidationErrorKind::MinContains {
                            min: self.min_contains,
                            actual: match_count,
                        }),
                );
            }
            if let Some(max) = self.max_contains {
                if match_count > max {
                    return Err(
                        ValidationErrorBuilder::new(instance_path.materialize(), Location::new())
                            .build(ValidationErrorKind::MaxContains {
                                max,
                                actual: match_count,
                            }),
                    );
                }
            }
            Ok(())
        } else {
            Ok(())
        }
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        if let Value::Array(arr) = instance {
            let mut match_count: u64 = 0;
            for (i, item) in arr.iter().enumerate() {
                if self.schema.is_valid(item, ctx) {
                    match_count += 1;
                    ctx.mark_item_evaluated(i);
                }
            }
            let kind = if match_count < self.min_contains {
                Some(ValidationErrorKind::MinContains {
                    min: self.min_contains,
                    actual: match_count,
                })
            } else if let Some(max) = self.max_contains {
                if match_count > max {
                    Some(ValidationErrorKind::MaxContains {
                        max,
                        actual: match_count,
                    })
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(kind) = kind {
                let err = ValidationErrorBuilder::new(instance_path.materialize(), Location::new())
                    .build(kind);
                return Box::new(core::iter::once(err));
            }
        }
        Box::new(core::iter::empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::{LazyLocation, Location};
    use serde_json::json;

    fn ctx() -> ValidationContext {
        ValidationContext::new()
    }

    #[test]
    fn contains_default_min_one() {
        let v = ContainsValidator::new(SchemaNode::AlwaysValid);
        assert!(v.is_valid(&json!([1]), &mut ctx()));
        assert!(!v.is_valid(&json!([]), &mut ctx()));
    }

    #[test]
    fn contains_with_min() {
        let v = ContainsValidator::new(SchemaNode::AlwaysValid).with_min(2);
        assert!(v.is_valid(&json!([1, 2, 3]), &mut ctx()));
        assert!(!v.is_valid(&json!([1]), &mut ctx()));
    }

    #[test]
    fn contains_with_max() {
        let v = ContainsValidator::new(SchemaNode::AlwaysValid).with_max(2);
        assert!(v.is_valid(&json!([1, 2]), &mut ctx()));
        assert!(!v.is_valid(&json!([1, 2, 3]), &mut ctx()));
    }

    #[test]
    fn contains_with_min_and_max() {
        let v = ContainsValidator::new(SchemaNode::AlwaysValid)
            .with_min(2)
            .with_max(4);
        assert!(v.is_valid(&json!([1, 2, 3]), &mut ctx()));
        assert!(!v.is_valid(&json!([1]), &mut ctx()));
        assert!(!v.is_valid(&json!([1, 2, 3, 4, 5]), &mut ctx()));
    }

    #[test]
    fn contains_marks_matched_items() {
        // `contains` marks matched items as evaluated for unevaluatedItems.
        let v = ContainsValidator::new(SchemaNode::AlwaysValid);
        let mut ctx = ctx();
        let _ = v.validate(&json!([1, 2, 3]), &LazyLocation::new(), &mut ctx);
        assert!(ctx.is_item_evaluated(0));
        assert!(ctx.is_item_evaluated(1));
        assert!(ctx.is_item_evaluated(2));
    }

    #[test]
    fn contains_non_array_instance() {
        let v = ContainsValidator::new(SchemaNode::AlwaysValid);
        assert!(v.is_valid(&json!("not array"), &mut ctx()));
        assert!(v.is_valid(&json!({"key": "value"}), &mut ctx()));
    }

    #[test]
    fn contains_selective_matching() {
        // Only integers match — use a type validator
        let int_schema = crate::node::SchemaNode::Validators {
            validators: vec![Box::new(crate::keywords::type_::TypeValidator::new(
                {
                    let mut set = crate::types::JsonTypeSet::new();
                    set.insert(crate::types::JsonType::Integer);
                    set
                },
                Location::new(),
            ))],
            schema_path: Location::new(),
        };
        let v = ContainsValidator::new(int_schema).with_min(1);
        assert!(v.is_valid(&json!([1, "a", "b"]), &mut ctx()));
        assert!(!v.is_valid(&json!(["a", "b", true]), &mut ctx()));
    }
}
