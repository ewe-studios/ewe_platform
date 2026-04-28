//! Stub: Schema compiler — will be implemented in Feature 3.

use crate::error::ValidationError;
use serde_json::Value;

/// Compile a JSON Schema into a validator tree.
pub fn compile(
    _schema: &Value,
    _registry: &crate::referencing::Registry,
    _options: &crate::options::ValidationOptions,
) -> Result<super::node::SchemaNode, ValidationError> {
    unimplemented!()
}
