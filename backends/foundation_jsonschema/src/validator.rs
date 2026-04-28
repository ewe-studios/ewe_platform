//! Validator — public wrapper around a compiled SchemaNode.
//!
//! WHY: Users interact with the `Validator` type, not `SchemaNode` directly.
//! This wraps the compiled tree with the public API.

use alloc::boxed::Box;

use crate::draft::Draft;
use crate::error::{ErrorIterator, ValidationError};
use crate::node::SchemaNode;
use crate::paths::LazyLocation;
use crate::referencing::Registry;
use serde_json::Value;

/// A compiled JSON Schema validator.
///
/// WHY: This is the public-facing type that wraps a `SchemaNode` and provides
/// the user-facing API. Created via `validator_for()` or `ValidationOptions::build()`.
///
/// IMMUTABLE and safe for concurrent use (`Send + Sync`).
pub struct Validator {
    root: SchemaNode,
    draft: Draft,
}

impl Validator {
    /// Create a new validator from a schema node and draft.
    pub(crate) fn new(root: SchemaNode, draft: Draft) -> Self {
        Self { root, draft }
    }

    /// Check if the instance is valid. Fast boolean result, no error details.
    pub fn is_valid(&self, instance: &Value) -> bool {
        let mut ctx = crate::keywords::ValidationContext::new();
        self.root.is_valid(instance, &mut ctx)
    }

    /// Validate and return the first error.
    pub fn validate(&self, instance: &Value) -> Result<(), ValidationError> {
        let mut ctx = crate::keywords::ValidationContext::new();
        let path = LazyLocation::new();
        self.root.validate(instance, &path, &mut ctx)
    }

    /// Return an iterator over all validation errors.
    pub fn iter_errors(&self, instance: &Value) -> ErrorIterator {
        let mut ctx = crate::keywords::ValidationContext::new();
        let path = LazyLocation::new();
        self.root.iter_errors(instance, &path, &mut ctx)
    }

    /// The draft this schema was compiled under.
    #[must_use]
    pub fn draft(&self) -> Draft {
        self.draft
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::Draft;
    use crate::node::SchemaNode;
    use serde_json::json;

    #[test]
    fn always_valid() {
        let v = Validator::new(SchemaNode::AlwaysValid, Draft::Draft7);
        assert!(v.is_valid(&json!("anything")));
    }

    #[test]
    fn always_invalid() {
        let v = Validator::new(
            SchemaNode::AlwaysInvalid {
                schema_path: crate::paths::Location::new(),
            },
            Draft::Draft7,
        );
        assert!(!v.is_valid(&json!("anything")));
    }

    #[test]
    fn draft_returns_correct() {
        let v = Validator::new(SchemaNode::AlwaysValid, Draft::Draft202012);
        assert_eq!(v.draft(), Draft::Draft202012);
    }
}
