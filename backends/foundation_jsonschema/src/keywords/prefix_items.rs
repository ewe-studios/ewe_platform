//! `prefixItems` (Draft 2020-12) — validates array items at specific positions.

use alloc::boxed::Box;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError};
use crate::node::SchemaNode;
use crate::paths::LazyLocation;

use super::{Validate, ValidationContext};

/// Validates array items at positional indices against corresponding schemas.
pub struct PrefixItemsValidator {
    items: Vec<SchemaNode>,
}

impl PrefixItemsValidator {
    /// Create with pre-compiled item schemas.
    #[must_use]
    pub fn new(items: Vec<SchemaNode>) -> Self {
        Self { items }
    }
}

impl Validate for PrefixItemsValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        if let Value::Array(arr) = instance {
            for (i, item) in arr.iter().enumerate().take(self.items.len()) {
                if !self.items[i].is_valid(item, ctx) {
                    return false;
                }
                ctx.mark_item_evaluated(i);
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
            for (i, item) in arr.iter().enumerate().take(self.items.len()) {
                let child_path = instance_path.push_index(i);
                self.items[i].validate(item, &child_path, ctx)?;
                ctx.mark_item_evaluated(i);
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
            for (i, item) in arr.iter().enumerate().take(self.items.len()) {
                let child_path = instance_path.push_index(i);
                for e in self.items[i].iter_errors(item, &child_path, ctx) {
                    errors.push(e);
                }
                ctx.mark_item_evaluated(i);
            }
        }
        Box::new(errors.into_iter())
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
    fn prefix_items_exact_match() {
        let v = PrefixItemsValidator::new(vec![SchemaNode::AlwaysValid, SchemaNode::AlwaysValid]);
        assert!(v.is_valid(&json!([1, 2]), &mut ctx()));
    }

    #[test]
    fn prefix_items_fewer_items() {
        let v = PrefixItemsValidator::new(vec![SchemaNode::AlwaysValid, SchemaNode::AlwaysValid]);
        assert!(v.is_valid(&json!([1]), &mut ctx()));
    }

    #[test]
    fn prefix_items_extra_items_allowed() {
        let v = PrefixItemsValidator::new(vec![SchemaNode::AlwaysValid]);
        // Items beyond prefixItems count are not validated by this validator
        assert!(v.is_valid(&json!([1, "anything", true, null]), &mut ctx()));
    }

    #[test]
    fn prefix_items_positional_failure() {
        let v = PrefixItemsValidator::new(vec![
            SchemaNode::AlwaysValid,
            SchemaNode::AlwaysInvalid {
                schema_path: Location::new(),
            },
        ]);
        assert!(!v.is_valid(&json!([1, 2]), &mut ctx()));
    }

    #[test]
    fn prefix_items_empty() {
        let v = PrefixItemsValidator::new(vec![]);
        assert!(v.is_valid(&json!([1, 2, 3]), &mut ctx()));
    }

    #[test]
    fn prefix_items_non_array() {
        let v = PrefixItemsValidator::new(vec![SchemaNode::AlwaysValid]);
        assert!(v.is_valid(&json!("not array"), &mut ctx()));
    }

    #[test]
    fn prefix_items_collects_errors() {
        let v = PrefixItemsValidator::new(vec![
            SchemaNode::AlwaysInvalid {
                schema_path: Location::new(),
            },
            SchemaNode::AlwaysInvalid {
                schema_path: Location::new(),
            },
        ]);
        let errors: Vec<_> = v
            .iter_errors(&json!(["a", "b"]), &LazyLocation::new(), &mut ctx())
            .collect();
        assert_eq!(errors.len(), 2);
    }
}
