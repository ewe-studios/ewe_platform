//! `$ref` — delegates validation to the referenced schema.
//!
//! WHY: `$ref` is the core mechanism for schema composition and reuse.
//! The referenced schema is compiled at construction time so validation
//! operates on a pre-compiled tree.

use alloc::string::String;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError};
use crate::node::SchemaNode;
use crate::paths::LazyLocation;

use super::{Validate, ValidationContext};

/// Validates by delegating to a compiled referenced schema.
pub struct RefValidator {
    #[allow(dead_code)]
    reference: String,
    #[allow(dead_code)]
    schema_path: crate::paths::Location,
    resolved_schema: SchemaNode,
}

impl RefValidator {
    /// Create a new ref validator with a pre-compiled referenced schema.
    #[must_use]
    pub fn new(
        reference: String,
        schema_path: crate::paths::Location,
        resolved_schema: SchemaNode,
    ) -> Self {
        Self {
            reference,
            schema_path,
            resolved_schema,
        }
    }
}

impl Validate for RefValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        self.resolved_schema.is_valid(instance, ctx)
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        self.resolved_schema.validate(instance, instance_path, ctx)
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        self.resolved_schema
            .iter_errors(instance, instance_path, ctx)
    }
}

/// `$dynamicRef` — dynamic scope reference resolution (Draft 2020-12).
///
/// WHY: `$dynamicRef` resolves to the outermost matching `$dynamicAnchor`
/// in the dynamic scope, enabling extensible recursive schemas.
pub struct DynamicRefValidator {
    #[allow(dead_code)]
    reference: String,
    #[allow(dead_code)]
    schema_path: crate::paths::Location,
    resolved_schema: SchemaNode,
}

impl DynamicRefValidator {
    /// Create a new dynamic ref validator with a pre-compiled referenced schema.
    #[must_use]
    pub fn new(
        reference: String,
        schema_path: crate::paths::Location,
        resolved_schema: SchemaNode,
    ) -> Self {
        Self {
            reference,
            schema_path,
            resolved_schema,
        }
    }
}

impl Validate for DynamicRefValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        self.resolved_schema.is_valid(instance, ctx)
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        self.resolved_schema.validate(instance, instance_path, ctx)
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        self.resolved_schema
            .iter_errors(instance, instance_path, ctx)
    }
}

/// `$recursiveRef` — recursive reference resolution (Draft 2019-09).
///
/// WHY: `$recursiveRef` walks the dynamic scope looking for schemas with
/// `$recursiveAnchor: true`, stopping at the outermost one.
pub struct RecursiveRefValidator {
    #[allow(dead_code)]
    reference: String,
    #[allow(dead_code)]
    schema_path: crate::paths::Location,
    resolved_schema: SchemaNode,
}

impl RecursiveRefValidator {
    /// Create a new recursive ref validator with a pre-compiled referenced schema.
    #[must_use]
    pub fn new(
        reference: String,
        schema_path: crate::paths::Location,
        resolved_schema: SchemaNode,
    ) -> Self {
        Self {
            reference,
            schema_path,
            resolved_schema,
        }
    }
}

impl Validate for RecursiveRefValidator {
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        self.resolved_schema.is_valid(instance, ctx)
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        self.resolved_schema.validate(instance, instance_path, ctx)
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        self.resolved_schema
            .iter_errors(instance, instance_path, ctx)
    }
}
