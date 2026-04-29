//! Meta-schema validation — validate schemas against their JSON Schema meta-schema.
//!
//! WHY: Schemas themselves can be validated against their meta-schema to ensure
//! correctness before compilation. This is useful for tooling, IDE integration,
//! and schema authoring workflows.
//!
//! WHAT: Functions to validate a schema document against its draft's meta-schema.
//!
//! HOW: Each draft has a canonical meta-schema (embedded in artefacts/). We compile
//! the meta-schema with the appropriate draft rules, then validate the input schema.

use serde_json::Value;

use crate::draft::Draft;
use crate::error::{ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::Location;
use crate::ValidationOptions;

/// Get the embedded meta-schema JSON for a draft.
///
/// WHY: The meta-schema is needed to validate schemas at runtime.
///
/// WHAT: Returns the parsed meta-schema `Value` for the given draft.
///
/// HOW: Uses `include_str!` to embed the artefact files at compile time.
#[must_use]
pub fn meta_schema(draft: Draft) -> Value {
    let json = match draft {
        Draft::Draft4 => include_str!("../artefacts/draft-04-schema.json"),
        Draft::Draft6 => include_str!("../artefacts/draft-06-schema.json"),
        Draft::Draft7 => include_str!("../artefacts/draft-07-schema.json"),
        Draft::Draft201909 => include_str!("../artefacts/draft-2019-09-schema.json"),
        Draft::Draft202012 => include_str!("../artefacts/draft-2020-12-schema.json"),
    };
    serde_json::from_str(json).expect("embedded meta-schema must be valid JSON")
}

/// Validate a schema against its meta-schema (auto-detect draft from `$schema`).
///
/// WHY: Users want to validate schemas without knowing which draft they target.
/// Auto-detection reads `$schema` and picks the right meta-schema.
///
/// WHAT: Returns `Ok(())` if the schema conforms to its meta-schema, or
/// `Err` with details about the first meta-schema violation.
///
/// HOW: Detects draft → compiles meta-schema → validates the input schema.
///
/// # Errors
///
/// Returns `Err` if draft detection fails (no `$schema` keyword), or if
/// the schema does not conform to the meta-schema.
pub fn validate_schema(schema: &Value) -> Result<(), ValidationError> {
    let draft = Draft::detect(schema).ok_or_else(|| {
        ValidationErrorBuilder::new(Location::new(), Location::new())
            .build(ValidationErrorKind::Schema {
                reason: "no $schema keyword found; cannot auto-detect draft for meta-schema validation".into(),
            })
    })?;
    validate_schema_with_draft(schema, draft)
}

/// Validate a schema against a specific draft's meta-schema.
///
/// WHY: When the draft is known explicitly, skip auto-detection and use
/// the specified meta-schema directly.
///
/// WHAT: Returns `Ok(())` if valid, or `Err` with meta-schema violations.
///
/// HOW: Compiles the draft's meta-schema and validates the input against it.
///
/// # Errors
///
/// Returns `Err` if the schema does not conform to the meta-schema.
pub fn validate_schema_with_draft(schema: &Value, draft: Draft) -> Result<(), ValidationError> {
    let meta = meta_schema(draft);
    let validator = ValidationOptions::new()
        .with_draft(draft)
        .build(&meta)?;

    if validator.is_valid(schema) {
        return Ok(());
    }

    // Collect the first error for reporting
    let mut errors = validator.iter_errors(schema);
    if let Some(err) = errors.next() {
        return Err(err);
    }

    Ok(())
}

/// Check if a schema conforms to its meta-schema (boolean result).
///
/// WHY: Quick validity check without error details.
///
/// WHAT: Returns `true` if the schema is valid, `false` otherwise.
///
/// HOW: Auto-detects draft, compiles meta-schema, checks validity.
///
/// # Errors
///
/// Returns `Err` if draft detection fails.
pub fn is_schema_valid(schema: &Value) -> Result<bool, ValidationError> {
    let draft = Draft::detect(schema).ok_or_else(|| {
        ValidationErrorBuilder::new(Location::new(), Location::new())
            .build(ValidationErrorKind::Schema {
                reason: "no $schema keyword found".into(),
            })
    })?;
    Ok(is_schema_valid_with_draft(schema, draft))
}

/// Check if a schema conforms to a specific draft's meta-schema (boolean result).
///
/// WHY: Quick validity check with explicit draft selection.
///
/// WHAT: Returns `true` if valid under the given draft's meta-schema.
#[must_use]
pub fn is_schema_valid_with_draft(schema: &Value, draft: Draft) -> bool {
    let meta = meta_schema(draft);
    let Ok(validator) = ValidationOptions::new().with_draft(draft).build(&meta) else {
        return false;
    };
    validator.is_valid(schema)
}

// ── Draft-specific sub-modules ────────────────────────────────────────

/// Meta-schema validation for Draft 4.
pub mod draft4 {
    use super::{Value, ValidationError, Draft};

    /// Validate a schema against the Draft 4 meta-schema.
    ///
    /// # Errors
    ///
    /// Returns an error if the schema does not conform to the Draft 4 meta-schema.
    pub fn validate(schema: &Value) -> Result<(), ValidationError> {
        super::validate_schema_with_draft(schema, Draft::Draft4)
    }

    /// Check if a schema conforms to the Draft 4 meta-schema.
    #[must_use]
    pub fn is_valid(schema: &Value) -> bool {
        super::is_schema_valid_with_draft(schema, Draft::Draft4)
    }
}

/// Meta-schema validation for Draft 6.
pub mod draft6 {
    use super::{Value, ValidationError, Draft};

    /// Validate a schema against the Draft 6 meta-schema.
    ///
    /// # Errors
    ///
    /// Returns an error if the schema does not conform to the Draft 6 meta-schema.
    pub fn validate(schema: &Value) -> Result<(), ValidationError> {
        super::validate_schema_with_draft(schema, Draft::Draft6)
    }

    /// Check if a schema conforms to the Draft 6 meta-schema.
    #[must_use]
    pub fn is_valid(schema: &Value) -> bool {
        super::is_schema_valid_with_draft(schema, Draft::Draft6)
    }
}

/// Meta-schema validation for Draft 7.
pub mod draft7 {
    use super::{Value, ValidationError, Draft};

    /// Validate a schema against the Draft 7 meta-schema.
    ///
    /// # Errors
    ///
    /// Returns an error if the schema does not conform to the Draft 7 meta-schema.
    pub fn validate(schema: &Value) -> Result<(), ValidationError> {
        super::validate_schema_with_draft(schema, Draft::Draft7)
    }

    /// Check if a schema conforms to the Draft 7 meta-schema.
    #[must_use]
    pub fn is_valid(schema: &Value) -> bool {
        super::is_schema_valid_with_draft(schema, Draft::Draft7)
    }
}

/// Meta-schema validation for Draft 2019-09.
pub mod draft201909 {
    use super::{Value, ValidationError, Draft};

    /// Validate a schema against the Draft 2019-09 meta-schema.
    ///
    /// # Errors
    ///
    /// Returns an error if the schema does not conform to the Draft 2019-09 meta-schema.
    pub fn validate(schema: &Value) -> Result<(), ValidationError> {
        super::validate_schema_with_draft(schema, Draft::Draft201909)
    }

    /// Check if a schema conforms to the Draft 2019-09 meta-schema.
    #[must_use]
    pub fn is_valid(schema: &Value) -> bool {
        super::is_schema_valid_with_draft(schema, Draft::Draft201909)
    }
}

/// Meta-schema validation for Draft 2020-12.
pub mod draft202012 {
    use super::{Value, ValidationError, Draft};

    /// Validate a schema against the Draft 2020-12 meta-schema.
    ///
    /// # Errors
    ///
    /// Returns an error if the schema does not conform to the Draft 2020-12 meta-schema.
    pub fn validate(schema: &Value) -> Result<(), ValidationError> {
        super::validate_schema_with_draft(schema, Draft::Draft202012)
    }

    /// Check if a schema conforms to the Draft 2020-12 meta-schema.
    #[must_use]
    pub fn is_valid(schema: &Value) -> bool {
        super::is_schema_valid_with_draft(schema, Draft::Draft202012)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_meta_schema_embedded() {
        // Ensure all draft meta-schemas parse correctly
        let _ = meta_schema(Draft::Draft4);
        let _ = meta_schema(Draft::Draft6);
        let _ = meta_schema(Draft::Draft7);
        let _ = meta_schema(Draft::Draft201909);
        let _ = meta_schema(Draft::Draft202012);
    }

    #[test]
    fn test_no_schema_keyword_fails_auto_detect() {
        let schema = json!({"type": "string"});
        assert!(validate_schema(&schema).is_err());
        assert!(is_schema_valid(&schema).is_err());
    }

    #[test]
    fn test_schema_uri_detection_works() {
        let s4 = json!({"$schema": "http://json-schema.org/draft-04/schema#"});
        assert_eq!(Draft::detect(&s4), Some(Draft::Draft4));
        let s7 = json!({"$schema": "http://json-schema.org/draft-07/schema#"});
        assert_eq!(Draft::detect(&s7), Some(Draft::Draft7));
        let s2020 = json!({"$schema": "https://json-schema.org/draft/2020-12/schema"});
        assert_eq!(Draft::detect(&s2020), Some(Draft::Draft202012));
    }
}
