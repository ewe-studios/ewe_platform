//! Structured validation error reporting via `foundation_errstacks`.
//!
//! WHY: When validation fails, users need to know exactly what went wrong,
//! where in the instance, and which schema rule caused the failure.
//!
//! WHAT: `ValidationErrorKind` enum for error categorization, `ValidationError`
//! as `ErrorTrace<ValidationErrorKind>`, `ValidationFailure` for programmatic
//! access to error + paths.
//!
//! HOW: Errors are created via `ValidationErrorBuilder` which attaches instance
//! and schema paths as opaque attachments. Display is provided by the
//! `ValidationErrorKind`'s Display impl.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_errstacks::{ErrorTrace, IntoErrorTrace};
use serde_json::Value;

use crate::paths::Location;
use crate::types::{JsonType, JsonTypeSet};

// Re-export ErrorTrace alias
/// Type alias for validation errors — full `foundation_errstacks` API.
pub type ValidationError = ErrorTrace<ValidationErrorKind>;

/// Lazy iterator over validation errors.
pub type ErrorIterator = Box<dyn Iterator<Item = ValidationError>>;

/// Categorization of all possible validation failures.
///
/// WHY: Each variant corresponds to a specific JSON Schema keyword or
/// validation rule that can fail. This enables programmatic error
/// categorization and filtering.
///
/// WHAT: A flat enum with one variant per validation failure type.
///
/// HOW: Created during validation. Paths are attached as opaque attachments
/// in the `ErrorTrace` wrapper, not stored here — this keeps the context
/// type minimal for pattern matching.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationErrorKind {
    // Type validation
    InvalidType {
        expected: JsonTypeSet,
        actual: JsonType,
    },

    // Enum/const
    InvalidConst {
        expected: Value,
    },
    InvalidEnum {
        options: Vec<Value>,
    },

    // String
    MinLength {
        min: u64,
        actual: u64,
    },
    MaxLength {
        max: u64,
        actual: u64,
    },
    Pattern {
        pattern: String,
    },
    Format {
        format: String,
    },

    // Number
    Minimum {
        limit: f64,
        actual: f64,
        exclusive: bool,
    },
    Maximum {
        limit: f64,
        actual: f64,
        exclusive: bool,
    },
    MultipleOf {
        multiple: f64,
        actual: f64,
    },

    // Object
    Required {
        property: String,
    },
    MinProperties {
        min: u64,
        actual: u64,
    },
    MaxProperties {
        max: u64,
        actual: u64,
    },
    AdditionalProperties {
        unexpected: Vec<String>,
    },
    PropertyNames {
        invalid_name: String,
    },

    // Array
    MinItems {
        min: u64,
        actual: u64,
    },
    MaxItems {
        max: u64,
        actual: u64,
    },
    UniqueItems {
        first: usize,
        second: usize,
    },
    Contains,
    MinContains {
        min: u64,
        actual: u64,
    },
    MaxContains {
        max: u64,
        actual: u64,
    },
    AdditionalItems {
        limit: usize,
    },

    // Composition
    Not,
    AnyOf,
    OneOfNotValid,
    OneOfMultipleValid,
    IfThenElse,

    // Reference
    RefNotFound {
        reference: String,
    },

    // Content
    ContentEncoding {
        encoding: String,
    },
    ContentMediaType {
        media_type: String,
    },
    ContentSchema,

    // Schema-level
    FalseSchema,

    // Unevaluated
    UnevaluatedProperties {
        properties: Vec<String>,
    },
    UnevaluatedItems {
        index: usize,
    },

    // Dependencies
    DependentRequired {
        property: String,
        missing: Vec<String>,
    },

    // Schema-level errors (registry/resolver)
    Schema {
        reason: String,
    },

    // Custom
    Custom {
        message: String,
    },
}

#[allow(clippy::too_many_lines)]
impl core::fmt::Display for ValidationErrorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidType { expected, actual } => {
                write!(f, "value is not of type(s) {expected}, got {actual}")
            }
            Self::InvalidConst { expected } => {
                write!(f, "value does not match constant {expected}")
            }
            Self::InvalidEnum { options } => {
                write!(f, "value is not one of the allowed options: {options:?}")
            }
            Self::MinLength { min, actual } => {
                write!(f, "string is too short (length {actual}, minimum {min})")
            }
            Self::MaxLength { max, actual } => {
                write!(f, "string is too long (length {actual}, maximum {max})")
            }
            Self::Pattern { pattern } => write!(f, "string does not match pattern '{pattern}'"),
            Self::Format { format } => write!(f, "value does not match format '{format}'"),
            Self::Minimum {
                limit,
                actual,
                exclusive: true,
            } => write!(f, "value {actual} is less than or equal to minimum {limit}"),
            Self::Minimum {
                limit,
                actual,
                exclusive: false,
            } => write!(f, "value {actual} is less than minimum {limit}"),
            Self::Maximum {
                limit,
                actual,
                exclusive: true,
            } => write!(
                f,
                "value {actual} is greater than or equal to maximum {limit}"
            ),
            Self::Maximum {
                limit,
                actual,
                exclusive: false,
            } => write!(f, "value {actual} is greater than maximum {limit}"),
            Self::MultipleOf { multiple, actual } => {
                write!(f, "value {actual} is not a multiple of {multiple}")
            }
            Self::Required { property } => write!(f, "'{property}' is a required property"),
            Self::MinProperties { min, actual } => write!(
                f,
                "object has too few properties (count {actual}, minimum {min})"
            ),
            Self::MaxProperties { max, actual } => write!(
                f,
                "object has too many properties (count {actual}, maximum {max})"
            ),
            Self::AdditionalProperties { unexpected } => write!(
                f,
                "additional properties are not allowed: {}",
                unexpected.join(", ")
            ),
            Self::PropertyNames { invalid_name } => {
                write!(f, "property name '{invalid_name}' is invalid")
            }
            Self::MinItems { min, actual } => {
                write!(f, "array has too few items (count {actual}, minimum {min})")
            }
            Self::MaxItems { max, actual } => write!(
                f,
                "array has too many items (count {actual}, maximum {max})"
            ),
            Self::UniqueItems { first, second } => write!(
                f,
                "array items at indices {first} and {second} are not unique"
            ),
            Self::Contains => write!(f, "array does not contain any matching item"),
            Self::MinContains { min, actual } => write!(
                f,
                "array contains too few matching items (count {actual}, minimum {min})"
            ),
            Self::MaxContains { max, actual } => write!(
                f,
                "array contains too many matching items (count {actual}, maximum {max})"
            ),
            Self::AdditionalItems { limit } => {
                write!(f, "array has additional items beyond index {limit}")
            }
            Self::Not => write!(f, "value matches the 'not' schema"),
            Self::AnyOf => write!(f, "value does not match any of the 'anyOf' schemas"),
            Self::OneOfNotValid => {
                write!(f, "value does not match exactly one of the 'oneOf' schemas")
            }
            Self::OneOfMultipleValid => {
                write!(f, "value matches more than one of the 'oneOf' schemas")
            }
            Self::IfThenElse => write!(f, "conditional validation failed (if/then/else)"),
            Self::RefNotFound { reference } => {
                write!(f, "reference '{reference}' could not be resolved")
            }
            Self::ContentEncoding { encoding } => {
                write!(f, "content encoding '{encoding}' is invalid")
            }
            Self::ContentMediaType { media_type } => {
                write!(f, "content media type '{media_type}' is invalid")
            }
            Self::ContentSchema => {
                write!(f, "content does not conform to contentSchema")
            }
            Self::FalseSchema => write!(f, "false schema always fails validation"),
            Self::UnevaluatedProperties { properties } => {
                write!(f, "unevaluated properties: {}", properties.join(", "))
            }
            Self::UnevaluatedItems { index } => write!(f, "unevaluated item at index {index}"),
            Self::DependentRequired { property, missing } => {
                write!(f, "property '{property}' requires: {}", missing.join(", "))
            }
            Self::Schema { reason } => write!(f, "schema error: {reason}"),
            Self::Custom { message } => write!(f, "{message}"),
        }
    }
}

impl ValidationErrorKind {
    /// Returns the JSON Schema keyword name that caused this error.
    ///
    /// WHY: Enables programmatic lookup of the failing keyword for
    /// tooling, IDE integration, and error filtering.
    ///
    /// WHAT: Returns `Some(keyword)` for standard keywords, `None` for
    /// non-keyword errors (Custom).
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub const fn keyword_name(&self) -> Option<&'static str> {
        match self {
            Self::InvalidType { .. } => Some("type"),
            Self::InvalidConst { .. } => Some("const"),
            Self::InvalidEnum { .. } => Some("enum"),
            Self::MinLength { .. } => Some("minLength"),
            Self::MaxLength { .. } => Some("maxLength"),
            Self::Pattern { .. } => Some("pattern"),
            Self::Format { .. } => Some("format"),
            Self::Minimum { .. } => Some("minimum"),
            Self::Maximum { .. } => Some("maximum"),
            Self::MultipleOf { .. } => Some("multipleOf"),
            Self::Required { .. } => Some("required"),
            Self::MinProperties { .. } => Some("minProperties"),
            Self::MaxProperties { .. } => Some("maxProperties"),
            Self::AdditionalProperties { .. } => Some("additionalProperties"),
            Self::PropertyNames { .. } => Some("propertyNames"),
            Self::MinItems { .. } => Some("minItems"),
            Self::MaxItems { .. } => Some("maxItems"),
            Self::UniqueItems { .. } => Some("uniqueItems"),
            Self::Contains { .. } => Some("contains"),
            Self::MinContains { .. } => Some("minContains"),
            Self::MaxContains { .. } => Some("maxContains"),
            Self::AdditionalItems { .. } => Some("additionalItems"),
            Self::Not { .. } => Some("not"),
            Self::AnyOf { .. } => Some("anyOf"),
            Self::OneOfNotValid { .. } | Self::OneOfMultipleValid { .. } => Some("oneOf"),
            Self::IfThenElse { .. } => Some("if"),
            Self::RefNotFound { .. } => Some("$ref"),
            Self::ContentEncoding { .. } => Some("contentEncoding"),
            Self::ContentMediaType { .. } => Some("contentMediaType"),
            Self::ContentSchema => Some("contentSchema"),
            Self::UnevaluatedProperties { .. } => Some("unevaluatedProperties"),
            Self::UnevaluatedItems { .. } => Some("unevaluatedItems"),
            Self::DependentRequired { .. } => Some("dependentRequired"),
            Self::FalseSchema { .. } | Self::Schema { .. } | Self::Custom { .. } => None,
        }
    }
}

impl core::error::Error for ValidationErrorKind {}

/// A validation error bundled with instance and schema paths.
///
/// WHY: Programmatic access to the error kind and paths without parsing
/// Display output or traversing `ErrorTrace` frames.
///
/// WHAT: A simple struct holding the error kind plus both paths.
///
/// HOW: Created via `to_failure()` which extracts paths from `ErrorTrace`
/// frames.
#[derive(Debug, Clone)]
pub struct ValidationFailure {
    /// The category of validation failure.
    pub kind: ValidationErrorKind,
    /// JSON Pointer path to the failing value in the instance.
    pub instance_path: Location,
    /// JSON Pointer path to the failing keyword in the schema.
    pub schema_path: Location,
}

/// Builder for creating validation errors with path attachments.
///
/// WHY: Every validation error needs `instance_path` and `schema_path` attached.
/// This builder provides a clean, ergonomic construction pattern that
/// replaces the old `ValidationError::new()` constructor.
///
/// WHAT: Collects paths then builds a `ValidationError` (`ErrorTrace`).
///
/// HOW: `build()` wraps the kind in an `ErrorTrace` and attaches both paths
/// as opaque attachments for programmatic extraction.
pub struct ValidationErrorBuilder {
    instance_path: Location,
    schema_path: Location,
}

impl ValidationErrorBuilder {
    /// Create a new builder with the given paths.
    #[must_use]
    pub fn new(instance_path: Location, schema_path: Location) -> Self {
        Self {
            instance_path,
            schema_path,
        }
    }

    /// Build a `ValidationError` from a kind, attaching paths.
    #[must_use]
    pub fn build(self, kind: ValidationErrorKind) -> ValidationError {
        kind.into_error_trace()
            .attach_opaque(self.instance_path)
            .attach_opaque(self.schema_path)
    }

    /// Build a `ValidationError` with an additional message attachment.
    pub fn build_with_message(
        self,
        kind: ValidationErrorKind,
        message: impl Into<String>,
    ) -> ValidationError {
        self.build(kind).attach(message.into())
    }
}

/// Build a `ValidationFailure` from a `ValidationError`.
///
/// WHY: Provides programmatic access to the error kind and paths without
/// traversing `ErrorTrace` frames manually.
///
/// WHAT: Returns `Some(ValidationFailure)` if both path attachments are
/// present, `None` otherwise.
///
/// HOW: Extracts the `ValidationErrorKind` context and the first two Location
/// attachments (`instance_path`, `schema_path`) from `ErrorTrace` frames.
///
/// # Panics
///
/// Never panics.
#[must_use]
pub fn to_failure(error: &ValidationError) -> Option<ValidationFailure> {
    let kind = error.current_context().clone();
    let mut locations: Vec<&Location> = error
        .frames()
        .filter_map(|f| f.as_any().downcast_ref::<Location>())
        .collect();
    if locations.len() < 2 {
        return None;
    }
    let instance_path = locations.remove(0).clone();
    let schema_path = locations.remove(0).clone();
    Some(ValidationFailure {
        kind,
        instance_path,
        schema_path,
    })
}
