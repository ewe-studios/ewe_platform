//! Vocabulary system for JSON Schema drafts.
//!
//! WHY: Drafts 2019-09 and 2020-12 introduce the concept of vocabularies —
//! named sets of keywords that a schema can declare via `$vocabulary`. The
//! compiler uses the active vocabulary set to decide which keywords to compile.
//!
//! WHAT: `VocabularySet` holds the set of active keyword names for a given
//! schema. `for_draft()` returns the default vocabulary for each draft.
//!
//! HOW: Each draft maps to a fixed set of keywords. Custom vocabularies
//! declared via `$vocabulary` in meta-schemas can extend or restrict this set.

use alloc::collections::BTreeSet;
use alloc::string::String;

use crate::draft::Draft;

/// A set of keywords active for a particular schema.
///
/// WHY: The compiler needs to know which keywords to recognize. Vocabularies
/// control this — a keyword not in the active vocabulary is ignored.
///
/// WHAT: A `BTreeSet<String>` of keyword names.
///
/// HOW: Constructed via `for_draft()` for the standard set, or built from
/// `$vocabulary` declarations in meta-schemas.
#[derive(Debug, Clone)]
pub struct VocabularySet {
    keywords: BTreeSet<String>,
}

impl VocabularySet {
    /// Create an empty vocabulary set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            keywords: BTreeSet::new(),
        }
    }

    /// Create the default vocabulary set for a draft.
    ///
    /// WHY: Each draft defines a fixed set of recognized keywords. This
    /// provides the default set used when no `$vocabulary` is declared.
    #[must_use]
    pub fn for_draft(draft: Draft) -> Self {
        let kw_list: &[&str] = match draft {
            Draft::Draft4 => DRAFT4_KEYWORDS,
            Draft::Draft6 => DRAFT6_KEYWORDS,
            Draft::Draft7 => DRAFT7_KEYWORDS,
            Draft::Draft201909 => DRAFT201909_KEYWORDS,
            Draft::Draft202012 => DRAFT202012_KEYWORDS,
        };
        let keywords = kw_list.iter().map(|s| String::from(*s)).collect();
        Self { keywords }
    }

    /// Check if a keyword is in the vocabulary.
    #[must_use]
    pub fn contains_keyword(&self, keyword: &str) -> bool {
        self.keywords.contains(keyword)
    }

    /// Insert a keyword.
    pub fn insert(&mut self, keyword: impl Into<String>) {
        self.keywords.insert(keyword.into());
    }

    /// The number of keywords in the vocabulary.
    #[must_use]
    pub fn len(&self) -> usize {
        self.keywords.len()
    }

    /// Whether the vocabulary is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keywords.is_empty()
    }
}

impl Default for VocabularySet {
    fn default() -> Self {
        Self::new()
    }
}

static DRAFT4_KEYWORDS: &[&str] = &[
    "type",
    "enum",
    "allOf",
    "anyOf",
    "oneOf",
    "not",
    "properties",
    "additionalProperties",
    "patternProperties",
    "required",
    "dependencies",
    "items",
    "additionalItems",
    "minItems",
    "maxItems",
    "uniqueItems",
    "minLength",
    "maxLength",
    "pattern",
    "format",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "$ref",
    "id",
    "definitions",
];

static DRAFT6_KEYWORDS: &[&str] = &[
    "type",
    "enum",
    "const",
    "allOf",
    "anyOf",
    "oneOf",
    "not",
    "properties",
    "additionalProperties",
    "patternProperties",
    "required",
    "dependencies",
    "propertyNames",
    "minProperties",
    "maxProperties",
    "items",
    "additionalItems",
    "contains",
    "minItems",
    "maxItems",
    "uniqueItems",
    "minLength",
    "maxLength",
    "pattern",
    "format",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "$ref",
    "$id",
    "definitions",
];

static DRAFT7_KEYWORDS: &[&str] = &[
    "type",
    "enum",
    "const",
    "allOf",
    "anyOf",
    "oneOf",
    "not",
    "if",
    "then",
    "else",
    "properties",
    "additionalProperties",
    "patternProperties",
    "required",
    "dependencies",
    "propertyNames",
    "minProperties",
    "maxProperties",
    "items",
    "additionalItems",
    "contains",
    "minItems",
    "maxItems",
    "uniqueItems",
    "minLength",
    "maxLength",
    "pattern",
    "format",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "contentEncoding",
    "contentMediaType",
    "readOnly",
    "writeOnly",
    "$ref",
    "$id",
    "definitions",
];

static DRAFT201909_KEYWORDS: &[&str] = &[
    "type",
    "enum",
    "const",
    "allOf",
    "anyOf",
    "oneOf",
    "not",
    "if",
    "then",
    "else",
    "properties",
    "additionalProperties",
    "patternProperties",
    "required",
    "minProperties",
    "maxProperties",
    "dependentRequired",
    "dependentSchemas",
    "propertyNames",
    "unevaluatedProperties",
    "items",
    "additionalItems",
    "contains",
    "minContains",
    "maxContains",
    "minItems",
    "maxItems",
    "uniqueItems",
    "unevaluatedItems",
    "minLength",
    "maxLength",
    "pattern",
    "format",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "contentEncoding",
    "contentMediaType",
    "contentSchema",
    "readOnly",
    "writeOnly",
    "$ref",
    "$recursiveRef",
    "$id",
    "$anchor",
    "$recursiveAnchor",
    "$defs",
    "definitions",
];

static DRAFT202012_KEYWORDS: &[&str] = &[
    "type",
    "enum",
    "const",
    "allOf",
    "anyOf",
    "oneOf",
    "not",
    "if",
    "then",
    "else",
    "properties",
    "additionalProperties",
    "patternProperties",
    "required",
    "minProperties",
    "maxProperties",
    "dependentRequired",
    "dependentSchemas",
    "propertyNames",
    "unevaluatedProperties",
    "prefixItems",
    "items",
    "contains",
    "minContains",
    "maxContains",
    "minItems",
    "maxItems",
    "uniqueItems",
    "unevaluatedItems",
    "minLength",
    "maxLength",
    "pattern",
    "format",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "contentEncoding",
    "contentMediaType",
    "contentSchema",
    "readOnly",
    "writeOnly",
    "$ref",
    "$dynamicRef",
    "$id",
    "$anchor",
    "$dynamicAnchor",
    "$defs",
    "definitions",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft7_has_if_then_else() {
        let vs = VocabularySet::for_draft(Draft::Draft7);
        assert!(vs.contains_keyword("if"));
        assert!(vs.contains_keyword("then"));
        assert!(vs.contains_keyword("else"));
    }

    #[test]
    fn draft4_no_if_then_else() {
        let vs = VocabularySet::for_draft(Draft::Draft4);
        assert!(!vs.contains_keyword("if"));
    }

    #[test]
    fn draft202012_has_prefix_items() {
        let vs = VocabularySet::for_draft(Draft::Draft202012);
        assert!(vs.contains_keyword("prefixItems"));
    }

    #[test]
    fn draft4_uses_id_not_dollar_id() {
        let vs = VocabularySet::for_draft(Draft::Draft4);
        assert!(vs.contains_keyword("id"));
        assert!(!vs.contains_keyword("$id"));
    }
}
