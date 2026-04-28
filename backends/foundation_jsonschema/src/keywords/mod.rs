//! Keywords and validators for all JSON Schema keywords.
//!
//! WHY: Each JSON Schema keyword (type, properties, minimum, $ref, etc.) needs
//! a corresponding validator struct that checks whether a JSON instance
//! satisfies the constraint.
//!
//! WHAT: `Validate` trait, `BoxedValidator` type alias, `BuiltinKeyword` enum.
//!
//! HOW: The compiler creates validator instances for each keyword found in
//! the schema. At validation time, each validator is called in sequence.

use crate::error::{ErrorIterator, ValidationError};
use crate::paths::LazyLocation;

use alloc::boxed::Box;
use serde_json::Value;

// Stub modules — will be implemented in subsequent features
pub(crate) mod custom;
pub(crate) mod all_of;
pub(crate) mod any_of;
pub(crate) mod one_of;
pub(crate) mod not_;
pub(crate) mod if_;
pub(crate) mod ref_;
pub(crate) mod properties;
pub(crate) mod additional_properties;
pub(crate) mod pattern_properties;
pub(crate) mod required;
pub(crate) mod property_names;
pub(crate) mod min_properties;
pub(crate) mod max_properties;
pub(crate) mod dependent_required;
pub(crate) mod dependent_schemas;
pub(crate) mod unevaluated_properties;
pub(crate) mod items;
pub(crate) mod prefix_items;
pub(crate) mod contains;
pub(crate) mod min_items;
pub(crate) mod max_items;
pub(crate) mod unique_items;
pub(crate) mod unevaluated_items;
pub(crate) mod min_length;
pub(crate) mod max_length;
pub(crate) mod pattern;
pub(crate) mod format;
pub(crate) mod minimum;
pub(crate) mod maximum;
pub(crate) mod exclusive_minimum;
pub(crate) mod exclusive_maximum;
pub(crate) mod multiple_of;
pub(crate) mod content;
pub(crate) mod legacy;
pub(crate) mod type_;
pub(crate) mod const_;
pub(crate) mod enum_;

/// The core validation trait implemented by all keyword validators.
///
/// WHY: Each JSON Schema keyword defines a constraint on the instance.
/// The Validate trait provides three levels of validation output:
/// - is_valid: fast boolean (no error details, no allocation)
/// - validate: first error only (early exit on first failure)
/// - iter_errors: all errors (for comprehensive reporting)
///
/// WHAT: A trait with three methods covering all validation output modes.
///
/// HOW: The compiler creates a Vec<BoxedValidator> for each schema node.
/// At validation time, each validator is called in sequence.
pub trait Validate: Send + Sync {
    /// Fast boolean validation. No error details, no path tracking.
    fn is_valid(&self, instance: &Value, ctx: &mut ValidationContext) -> bool;

    /// Validate and return the first error encountered.
    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError>;

    /// Return an iterator over all validation errors.
    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator;
}

/// Type alias for a heap-allocated validator.
pub type BoxedValidator = Box<dyn Validate>;

/// Enum of all recognized JSON Schema keywords for lookup during compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinKeyword {
    // Type
    Type,
    Const,
    Enum,
    // Logical
    AllOf,
    AnyOf,
    OneOf,
    Not,
    If,
    Then,
    Else,
    // Reference
    Ref,
    DynamicRef,
    RecursiveRef,
    // Object
    Properties,
    Required,
    AdditionalProperties,
    PatternProperties,
    PropertyNames,
    MinProperties,
    MaxProperties,
    DependentRequired,
    DependentSchemas,
    UnevaluatedProperties,
    Dependencies,
    // Array
    Items,
    AdditionalItems,
    PrefixItems,
    Contains,
    MinItems,
    MaxItems,
    UniqueItems,
    UnevaluatedItems,
    // String
    MinLength,
    MaxLength,
    Pattern,
    Format,
    // Number
    Minimum,
    Maximum,
    ExclusiveMinimum,
    ExclusiveMaximum,
    MultipleOf,
    // Content
    ContentEncoding,
    ContentMediaType,
    ContentSchema,
}

impl BuiltinKeyword {
    /// Returns the keyword name as a string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Const => "const",
            Self::Enum => "enum",
            Self::AllOf => "allOf",
            Self::AnyOf => "anyOf",
            Self::OneOf => "oneOf",
            Self::Not => "not",
            Self::If => "if",
            Self::Then => "then",
            Self::Else => "else",
            Self::Ref => "$ref",
            Self::DynamicRef => "$dynamicRef",
            Self::RecursiveRef => "$recursiveRef",
            Self::Properties => "properties",
            Self::Required => "required",
            Self::AdditionalProperties => "additionalProperties",
            Self::PatternProperties => "patternProperties",
            Self::PropertyNames => "propertyNames",
            Self::MinProperties => "minProperties",
            Self::MaxProperties => "maxProperties",
            Self::DependentRequired => "dependentRequired",
            Self::DependentSchemas => "dependentSchemas",
            Self::UnevaluatedProperties => "unevaluatedProperties",
            Self::Dependencies => "dependencies",
            Self::Items => "items",
            Self::AdditionalItems => "additionalItems",
            Self::PrefixItems => "prefixItems",
            Self::Contains => "contains",
            Self::MinItems => "minItems",
            Self::MaxItems => "maxItems",
            Self::UniqueItems => "uniqueItems",
            Self::UnevaluatedItems => "unevaluatedItems",
            Self::MinLength => "minLength",
            Self::MaxLength => "maxLength",
            Self::Pattern => "pattern",
            Self::Format => "format",
            Self::Minimum => "minimum",
            Self::Maximum => "maximum",
            Self::ExclusiveMinimum => "exclusiveMinimum",
            Self::ExclusiveMaximum => "exclusiveMaximum",
            Self::MultipleOf => "multipleOf",
            Self::ContentEncoding => "contentEncoding",
            Self::ContentMediaType => "contentMediaType",
            Self::ContentSchema => "contentSchema",
        }
    }

    /// Parse a keyword name into a `BuiltinKeyword`.
    ///
    /// WHY: The compiler needs to match schema object keys against known keywords.
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "type" => Self::Type,
            "const" => Self::Const,
            "enum" => Self::Enum,
            "allOf" => Self::AllOf,
            "anyOf" => Self::AnyOf,
            "oneOf" => Self::OneOf,
            "not" => Self::Not,
            "if" => Self::If,
            "then" => Self::Then,
            "else" => Self::Else,
            "$ref" => Self::Ref,
            "$dynamicRef" => Self::DynamicRef,
            "$recursiveRef" => Self::RecursiveRef,
            "properties" => Self::Properties,
            "required" => Self::Required,
            "additionalProperties" => Self::AdditionalProperties,
            "patternProperties" => Self::PatternProperties,
            "propertyNames" => Self::PropertyNames,
            "minProperties" => Self::MinProperties,
            "maxProperties" => Self::MaxProperties,
            "dependentRequired" => Self::DependentRequired,
            "dependentSchemas" => Self::DependentSchemas,
            "unevaluatedProperties" => Self::UnevaluatedProperties,
            "dependencies" => Self::Dependencies,
            "items" => Self::Items,
            "additionalItems" => Self::AdditionalItems,
            "prefixItems" => Self::PrefixItems,
            "contains" => Self::Contains,
            "minItems" => Self::MinItems,
            "maxItems" => Self::MaxItems,
            "uniqueItems" => Self::UniqueItems,
            "unevaluatedItems" => Self::UnevaluatedItems,
            "minLength" => Self::MinLength,
            "maxLength" => Self::MaxLength,
            "pattern" => Self::Pattern,
            "format" => Self::Format,
            "minimum" => Self::Minimum,
            "maximum" => Self::Maximum,
            "exclusiveMinimum" => Self::ExclusiveMinimum,
            "exclusiveMaximum" => Self::ExclusiveMaximum,
            "multipleOf" => Self::MultipleOf,
            "contentEncoding" => Self::ContentEncoding,
            "contentMediaType" => Self::ContentMediaType,
            "contentSchema" => Self::ContentSchema,
            _ => return None,
        })
    }
}

// Re-export ValidationContext from the validation_context module.
// This allows keyword validators to use it without a circular import.
pub use crate::validation_context::ValidationContext;
