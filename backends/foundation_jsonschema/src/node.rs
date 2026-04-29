//! `SchemaNode` — compiled representation of a single schema.
//!
//! WHY: After compilation, validation operates on `SchemaNode` trees rather
//! than raw JSON. This separates the cost of schema analysis from validation.

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::error::{ErrorIterator, ValidationError};
use crate::keywords::{BoxedValidator, ValidationContext};
use crate::paths::{LazyLocation, Location};

use foundation_errstacks::IntoErrorTrace;
use serde_json::Value;

/// A compiled schema node — the fundamental unit of the validator tree.
///
/// WHY: After compilation, validation operates on `SchemaNode` trees rather
/// than raw JSON. This separates the cost of schema analysis from validation.
pub enum SchemaNode {
    /// Schema is boolean `true` — validates everything.
    AlwaysValid,
    /// Schema is boolean `false` — validates nothing.
    #[allow(dead_code)]
    AlwaysInvalid { schema_path: Location },
    /// Schema with one or more keyword validators.
    Validators {
        validators: Vec<BoxedValidator>,
        #[allow(dead_code)]
        schema_path: Location,
    },
}

impl SchemaNode {
    /// Fast boolean validation.
    pub fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool {
        match self {
            Self::AlwaysValid => true,
            Self::AlwaysInvalid { .. } => false,
            Self::Validators { validators, .. } => {
                validators.iter().all(|v| v.is_valid(instance, ctx))
            }
        }
    }

    /// Validate and return the first error.
    pub fn validate(
        &self,
        instance: &Value,
        path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        match self {
            Self::AlwaysValid => Ok(()),
            Self::AlwaysInvalid { schema_path: _ } => Err(
                crate::error::ValidationErrorKind::FalseSchema.into_error_trace()
            ),
            Self::Validators { validators, .. } => {
                for v in validators {
                    v.validate(instance, path, ctx)?;
                }
                Ok(())
            }
        }
    }

    /// Return an iterator over all validation errors.
    pub fn iter_errors(
        &self,
        instance: &Value,
        path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        match self {
            Self::AlwaysValid => Box::new(core::iter::empty()),
            Self::AlwaysInvalid { schema_path: _ } => {
                Box::new(core::iter::once(
                    crate::error::ValidationErrorKind::FalseSchema.into_error_trace()
                ))
            }
            Self::Validators { validators, .. } => {
                let mut errors: Vec<ValidationError> = Vec::new();
                for v in validators {
                    for e in v.iter_errors(instance, path, ctx) {
                        errors.push(e);
                    }
                }
                Box::new(errors.into_iter())
            }
        }
    }
}
