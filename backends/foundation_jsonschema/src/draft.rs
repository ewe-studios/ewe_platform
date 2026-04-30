//! JSON Schema draft version identification.
//!
//! WHY: Different JSON Schema drafts have different keywords, behaviors, and
//! ID extraction rules. The compiler needs to know which draft a schema uses
//! to apply the correct validation rules.
//!
//! WHAT: `Draft` enum with 5 variants plus detection from `$schema` URIs.
//!
//! HOW: `Draft::detect()` reads the `$schema` keyword from a schema document
//! and maps it to the appropriate variant via `from_schema_uri()`.

use serde_json::Value;

/// JSON Schema specification version.
///
/// WHY: Each draft has distinct keyword semantics, ID handling rules, and
/// supported features. The validator must know which draft a schema conforms
/// to for correct validation.
///
/// WHAT: Five variants representing the major JSON Schema drafts.
///
/// HOW: Detected automatically from `$schema` URI, or set explicitly via
/// `ValidationOptions::with_draft()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Draft {
    /// JSON Schema Draft 4 (released 2013).
    ///
    /// Uses "id" (not "$id") for schema identification.
    /// No $anchor, $recursiveRef, or $dynamicRef support.
    Draft4,
    /// JSON Schema Draft 6 (released 2017).
    ///
    /// Introduces "$id", "const", "contains", and exclusiveMinimum/Maximum
    /// as numeric values (not booleans).
    Draft6,
    /// JSON Schema Draft 7 (released 2018).
    ///
    /// Adds "if/then/else", "readOnly", "writeOnly".
    Draft7,
    /// JSON Schema Draft 2019-09 (released 2019).
    ///
    /// Introduces vocabularies, "$recursiveRef"/"$recursiveAnchor",
    /// and "contentSchema".
    Draft201909,
    /// JSON Schema Draft 2020-12 (released 2020).
    ///
    /// Replaces $recursiveRef with "$dynamicRef"/"$dynamicAnchor",
    /// splits "items" into "prefixItems" + "items", and introduces
    /// "$defs" (replaces "definitions").
    Draft202012,
}

impl Draft {
    /// The default draft when none is specified.
    ///
    /// WHY: JSON Schema spec recommends using the latest draft as the default.
    pub const DEFAULT: Draft = Draft::Draft202012;

    /// Detect the draft from a schema's `$schema` keyword.
    ///
    /// WHY: Schemas often declare their draft via the `$schema` property.
    /// This allows automatic draft selection without user configuration.
    ///
    /// WHAT: Returns `Some(Draft)` if `$schema` is present and recognized,
    /// `None` otherwise.
    ///
    /// HOW: Reads the `$schema` string value and passes it to `from_schema_uri()`.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn detect(schema: &Value) -> Option<Self> {
        match schema {
            Value::Object(obj) => {
                if let Some(Value::String(uri)) = obj.get("$schema") {
                    Self::from_schema_uri(uri)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Parse a `$schema` URI string into a Draft.
    ///
    /// WHY: Schemas may use different URI formats — with or without the
    /// trailing "#", with "http" or "https". This normalizes all variants.
    ///
    /// WHAT: Returns `Some(Draft)` for recognized URIs, `None` for unknown.
    ///
    /// HOW: Matches against known schema URIs, stripping trailing "#" and
    /// normalizing http/https.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn from_schema_uri(uri: &str) -> Option<Self> {
        // Normalize: strip trailing "#", normalize http/https
        let normalized = uri.strip_suffix('#').unwrap_or(uri);

        if normalized == "http://json-schema.org/draft-04/schema"
            || normalized == "https://json-schema.org/draft-04/schema"
        {
            return Some(Draft::Draft4);
        }

        if normalized == "http://json-schema.org/draft-06/schema"
            || normalized == "https://json-schema.org/draft-06/schema"
        {
            return Some(Draft::Draft6);
        }

        if normalized == "http://json-schema.org/draft-07/schema"
            || normalized == "https://json-schema.org/draft-07/schema"
        {
            return Some(Draft::Draft7);
        }

        if normalized == "https://json-schema.org/draft/2019-09/schema" {
            return Some(Draft::Draft201909);
        }

        if normalized == "https://json-schema.org/draft/2020-12/schema" {
            return Some(Draft::Draft202012);
        }

        None
    }

    /// The canonical `$schema` URI for this draft.
    ///
    /// WHY: Meta-schema validation and schema identification need the canonical
    /// URI form.
    #[must_use]
    pub const fn schema_uri(&self) -> &'static str {
        match self {
            Draft::Draft4 => "http://json-schema.org/draft-04/schema#",
            Draft::Draft6 => "http://json-schema.org/draft-06/schema#",
            Draft::Draft7 => "http://json-schema.org/draft-07/schema#",
            Draft::Draft201909 => "https://json-schema.org/draft/2019-09/schema",
            Draft::Draft202012 => "https://json-schema.org/draft/2020-12/schema",
        }
    }

    /// The ID keyword name for this draft.
    ///
    /// WHY: Draft 4 uses "id" while Draft 6+ use "$id". The referencing
    /// engine needs to know which keyword to look for when extracting
    /// schema identifiers.
    #[must_use]
    pub const fn id_keyword(&self) -> &'static str {
        match self {
            Draft::Draft4 => "id",
            _ => "$id",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_draft_detect_draft202012() {
        let schema = json!({"$schema": "https://json-schema.org/draft/2020-12/schema"});
        assert_eq!(Draft::detect(&schema), Some(Draft::Draft202012));
    }

    #[test]
    fn test_draft_detect_draft201909() {
        let schema = json!({"$schema": "https://json-schema.org/draft/2019-09/schema"});
        assert_eq!(Draft::detect(&schema), Some(Draft::Draft201909));
    }

    #[test]
    fn test_draft_detect_draft7() {
        let schema = json!({"$schema": "http://json-schema.org/draft-07/schema#"});
        assert_eq!(Draft::detect(&schema), Some(Draft::Draft7));
    }

    #[test]
    fn test_draft_detect_draft6() {
        let schema = json!({"$schema": "http://json-schema.org/draft-06/schema#"});
        assert_eq!(Draft::detect(&schema), Some(Draft::Draft6));
    }

    #[test]
    fn test_draft_detect_draft4() {
        let schema = json!({"$schema": "http://json-schema.org/draft-04/schema#"});
        assert_eq!(Draft::detect(&schema), Some(Draft::Draft4));
    }

    #[test]
    fn test_draft_detect_absent() {
        let schema = json!({"type": "string"});
        assert_eq!(Draft::detect(&schema), None);
    }

    #[test]
    fn test_draft_detect_unknown() {
        let schema = json!({"$schema": "http://example.com/unknown"});
        assert_eq!(Draft::detect(&schema), None);
    }

    #[test]
    fn test_draft_from_schema_uri_no_trailing_hash() {
        assert_eq!(
            Draft::from_schema_uri("http://json-schema.org/draft-07/schema"),
            Some(Draft::Draft7)
        );
    }

    #[test]
    fn test_draft_from_schema_uri_with_hash() {
        assert_eq!(
            Draft::from_schema_uri("http://json-schema.org/draft-07/schema#"),
            Some(Draft::Draft7)
        );
    }

    #[test]
    fn test_draft_from_schema_uri_https() {
        assert_eq!(
            Draft::from_schema_uri("https://json-schema.org/draft-07/schema"),
            Some(Draft::Draft7)
        );
    }

    #[test]
    fn test_draft_from_schema_uri_unknown() {
        assert_eq!(Draft::from_schema_uri("http://example.com/unknown"), None);
    }

    #[test]
    fn test_draft_schema_uri() {
        assert_eq!(
            Draft::Draft4.schema_uri(),
            "http://json-schema.org/draft-04/schema#"
        );
        assert_eq!(
            Draft::Draft202012.schema_uri(),
            "https://json-schema.org/draft/2020-12/schema"
        );
    }

    #[test]
    fn test_draft_id_keyword() {
        assert_eq!(Draft::Draft4.id_keyword(), "id");
        assert_eq!(Draft::Draft6.id_keyword(), "$id");
        assert_eq!(Draft::Draft7.id_keyword(), "$id");
        assert_eq!(Draft::Draft201909.id_keyword(), "$id");
        assert_eq!(Draft::Draft202012.id_keyword(), "$id");
    }

    #[test]
    fn test_draft_default() {
        assert_eq!(Draft::DEFAULT, Draft::Draft202012);
    }
}
