//! JSON Schema resource types — a schema document paired with its draft.
//!
//! WHY: The referencing engine needs to know both the contents of a schema
//! and which draft it conforms to, because ID extraction, anchor extraction,
//! and sub-resource identification rules differ per draft.
//!
//! WHAT: `Resource` (owned) and `ResourceRef` (borrowed) pair a JSON value
//! with a `Draft`.
//!
//! HOW: `Resource::from_contents()` auto-detects the draft from `$schema`.
//! `ResourceRef::id()` uses the draft's `id_keyword()` to extract the correct
//! ID field.

use serde_json::Value;

use crate::draft::Draft;

/// An owned JSON Schema resource (document + draft).
///
/// WHY: The registry stores owned resources for schemas that were retrieved
/// externally or provided by the user.
///
/// WHAT: Wraps a `serde_json::Value` with its associated `Draft`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    contents: Value,
    draft: Draft,
}

/// A borrowed reference to a JSON Schema resource.
///
/// WHY: During traversal and resolution, we reference resources stored in
/// the registry without cloning. `Copy` makes passing these around cheap.
///
/// WHAT: References a `&Value` with its associated `Draft`.
#[derive(Debug, Clone, Copy)]
pub struct ResourceRef<'a> {
    contents: &'a Value,
    draft: Draft,
}

impl Resource {
    /// Create a resource with auto-detected draft.
    ///
    /// WHY: Most callers want the draft inferred from `$schema`.
    ///
    /// WHAT: Detects the draft from the `$schema` keyword. Falls back to
    /// `Draft::DEFAULT` if absent or unrecognized.
    #[must_use]
    pub fn from_contents(value: Value) -> Self {
        let draft = Draft::detect(&value).unwrap_or(Draft::DEFAULT);
        Self {
            contents: value,
            draft,
        }
    }

    /// Create with explicit draft.
    #[must_use]
    pub fn with_draft(value: Value, draft: Draft) -> Self {
        Self { contents: value, draft }
    }

    /// Get the JSON contents.
    #[must_use]
    pub fn contents(&self) -> &Value {
        &self.contents
    }

    /// Get the draft.
    #[must_use]
    pub fn draft(&self) -> Draft {
        self.draft
    }

    /// Get a borrowed reference.
    #[must_use]
    pub fn as_ref(&self) -> ResourceRef<'_> {
        ResourceRef {
            contents: &self.contents,
            draft: self.draft,
        }
    }

    /// Decompose into draft and contents.
    #[must_use]
    pub fn into_inner(self) -> (Draft, Value) {
        (self.draft, self.contents)
    }
}

impl<'a> ResourceRef<'a> {
    /// Create a new resource reference.
    #[must_use]
    pub fn new(contents: &'a Value, draft: Draft) -> Self {
        Self { contents, draft }
    }

    /// Create with auto-detected draft.
    #[must_use]
    pub fn from_contents(contents: &'a Value) -> Self {
        let draft = Draft::detect(contents).unwrap_or(Draft::DEFAULT);
        Self { contents, draft }
    }

    /// Extract the ID from this resource (draft-specific).
    ///
    /// WHY: Draft 4 uses `"id"` while all others use `"$id"`. IDs that start
    /// with `#` are anchors, not resource IDs — those are stripped.
    ///
    /// WHAT: Returns the `$id`/`id` value, stripped of trailing `#`.
    #[must_use]
    pub fn id(&self) -> Option<&'a str> {
        if let Value::Object(obj) = self.contents {
            let key = self.draft.id_keyword();
            obj.get(key)
                .and_then(Value::as_str)
                .map(|id| id.trim_end_matches('#'))
        } else {
            None
        }
    }

    /// Get the contents.
    #[must_use]
    pub fn contents(&self) -> &'a Value {
        self.contents
    }

    /// Get the draft.
    #[must_use]
    pub fn draft(&self) -> Draft {
        self.draft
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resource_from_contents_detects_draft() {
        let r = Resource::from_contents(json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "string"
        }));
        assert_eq!(r.draft(), Draft::Draft202012);
    }

    #[test]
    fn resource_from_contents_defaults() {
        let r = Resource::from_contents(json!({"type": "string"}));
        assert_eq!(r.draft(), Draft::DEFAULT);
    }

    #[test]
    fn resource_with_draft() {
        let r = Resource::with_draft(json!({"type": "string"}), Draft::Draft4);
        assert_eq!(r.draft(), Draft::Draft4);
    }

    #[test]
    fn resource_ref_id_modern() {
        let schema = json!({"$id": "http://example.com/schema"});
        let rr = ResourceRef::new(&schema, Draft::Draft202012);
        assert_eq!(rr.id(), Some("http://example.com/schema"));
    }

    #[test]
    fn resource_ref_id_draft4() {
        let schema = json!({"id": "http://example.com/schema"});
        let rr = ResourceRef::new(&schema, Draft::Draft4);
        assert_eq!(rr.id(), Some("http://example.com/schema"));
    }

    #[test]
    fn resource_ref_id_strips_trailing_hash() {
        let schema = json!({"$id": "http://example.com/schema#"});
        let rr = ResourceRef::new(&schema, Draft::Draft7);
        assert_eq!(rr.id(), Some("http://example.com/schema"));
    }

    #[test]
    fn resource_ref_id_none_for_boolean() {
        let schema = json!(true);
        let rr = ResourceRef::new(&schema, Draft::Draft202012);
        assert_eq!(rr.id(), None);
    }

    #[test]
    fn resource_ref_from_contents() {
        let schema = json!({"$schema": "http://json-schema.org/draft-07/schema#", "type": "object"});
        let rr = ResourceRef::from_contents(&schema);
        assert_eq!(rr.draft(), Draft::Draft7);
    }
}
