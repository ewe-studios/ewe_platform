//! Anchor types for named sub-schema resolution.
//!
//! WHY: JSON Schema uses anchors (`$anchor`, `$dynamicAnchor`, legacy `#fragment`
//! in `$id`/`id`) to name sub-schemas so they can be referenced by name instead
//! of JSON Pointer. Dynamic anchors have special resolution semantics that walk
//! the dynamic scope.
//!
//! WHAT: `Anchor` enum (Default vs Dynamic), `AnchorIter` for yielding 0–2
//! anchors from a schema object without heap allocation.
//!
//! HOW: `AnchorIter` is a small enum that avoids a `Vec` for the common case
//! of 0–2 anchors per schema.

use serde_json::Value;

use crate::draft::Draft;

use super::resource::ResourceRef;

/// An anchor within a JSON Schema resource.
///
/// WHY: Anchors are the named-reference mechanism in JSON Schema. Default
/// anchors resolve directly; dynamic anchors walk the evaluation scope.
///
/// WHAT: Two variants — `Default` for plain `$anchor` and `Dynamic` for
/// `$dynamicAnchor`.
#[derive(Debug, Clone, Copy)]
pub enum Anchor<'a> {
    /// A regular anchor (`$anchor`, or legacy `#fragment` in `$id`/`id`).
    Default {
        /// The anchor name.
        name: &'a str,
        /// The resource this anchor points to.
        resource: ResourceRef<'a>,
    },
    /// A dynamic anchor (`$dynamicAnchor` in Draft 2020-12).
    Dynamic {
        /// The anchor name.
        name: &'a str,
        /// The resource this anchor points to.
        resource: ResourceRef<'a>,
    },
}

impl<'a> Anchor<'a> {
    /// The anchor's name.
    #[must_use]
    pub fn name(&self) -> &'a str {
        match self {
            Anchor::Default { name, .. } | Anchor::Dynamic { name, .. } => name,
        }
    }

    /// The resource this anchor points to.
    #[must_use]
    pub fn resource(&self) -> ResourceRef<'a> {
        match self {
            Anchor::Default { resource, .. } | Anchor::Dynamic { resource, .. } => *resource,
        }
    }
}

/// An iterator over 0, 1, or 2 anchors — avoids a `Vec` allocation.
///
/// WHY: Most schema objects have 0 or 1 anchors. Draft 2020-12 can have both
/// `$anchor` and `$dynamicAnchor` on the same object (2 anchors). A small
/// enum is more efficient than a heap-allocated `Vec`.
pub enum AnchorIter<'a> {
    /// No anchors.
    Empty,
    /// One anchor.
    One(Anchor<'a>),
    /// Two anchors (e.g., `$anchor` + `$dynamicAnchor`).
    Two(Anchor<'a>, Anchor<'a>),
}

impl<'a> Iterator for AnchorIter<'a> {
    type Item = Anchor<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match core::mem::replace(self, AnchorIter::Empty) {
            AnchorIter::Empty => None,
            AnchorIter::One(anchor) => Some(anchor),
            AnchorIter::Two(first, second) => {
                *self = AnchorIter::One(second);
                Some(first)
            }
        }
    }
}

/// Extract anchors from a Draft 2020-12 schema object.
///
/// Recognizes `$anchor` (Default) and `$dynamicAnchor` (Dynamic).
pub fn anchors_202012(draft: Draft, contents: &Value) -> AnchorIter<'_> {
    let Some(schema) = contents.as_object() else {
        return AnchorIter::Empty;
    };

    let default_anchor =
        schema
            .get("$anchor")
            .and_then(Value::as_str)
            .map(|name| Anchor::Default {
                name,
                resource: ResourceRef::new(contents, draft),
            });

    let dynamic_anchor = schema
        .get("$dynamicAnchor")
        .and_then(Value::as_str)
        .map(|name| Anchor::Dynamic {
            name,
            resource: ResourceRef::new(contents, draft),
        });

    match (default_anchor, dynamic_anchor) {
        (Some(d), Some(dyn_a)) => AnchorIter::Two(d, dyn_a),
        (Some(d), None) => AnchorIter::One(d),
        (None, Some(dyn_a)) => AnchorIter::One(dyn_a),
        (None, None) => AnchorIter::Empty,
    }
}

/// Extract anchors from a Draft 2019-09 schema object.
///
/// Recognizes `$anchor` (Default) only. `$recursiveAnchor` is handled
/// separately during recursive ref resolution.
pub fn anchors_201909(draft: Draft, contents: &Value) -> AnchorIter<'_> {
    match contents
        .as_object()
        .and_then(|schema| schema.get("$anchor"))
        .and_then(Value::as_str)
    {
        Some(name) => AnchorIter::One(Anchor::Default {
            name,
            resource: ResourceRef::new(contents, draft),
        }),
        None => AnchorIter::Empty,
    }
}

/// Extract legacy anchors from Draft 6/7 schemas.
///
/// In these drafts, `$id` values starting with '#' define anchors.
pub fn legacy_anchor_in_dollar_id(draft: Draft, contents: &Value) -> AnchorIter<'_> {
    match contents
        .as_object()
        .and_then(|schema| schema.get("$id"))
        .and_then(Value::as_str)
        .and_then(|id| id.strip_prefix('#'))
    {
        Some(id) => AnchorIter::One(Anchor::Default {
            name: id,
            resource: ResourceRef::new(contents, draft),
        }),
        None => AnchorIter::Empty,
    }
}

/// Extract legacy anchors from Draft 4 schemas.
///
/// In Draft 4, `id` values starting with '#' define anchors.
pub fn legacy_anchor_in_id(draft: Draft, contents: &Value) -> AnchorIter<'_> {
    match contents
        .as_object()
        .and_then(|schema| schema.get("id"))
        .and_then(Value::as_str)
        .and_then(|id| id.strip_prefix('#'))
    {
        Some(id) => AnchorIter::One(Anchor::Default {
            name: id,
            resource: ResourceRef::new(contents, draft),
        }),
        None => AnchorIter::Empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn anchor_202012_both() {
        let schema = json!({"$anchor": "foo", "$dynamicAnchor": "bar"});
        let mut iter = anchors_202012(Draft::Draft202012, &schema);
        let first = iter.next().unwrap();
        assert_eq!(first.name(), "foo");
        assert!(matches!(first, Anchor::Default { .. }));
        let second = iter.next().unwrap();
        assert_eq!(second.name(), "bar");
        assert!(matches!(second, Anchor::Dynamic { .. }));
        assert!(iter.next().is_none());
    }

    #[test]
    fn anchor_202012_only_dynamic() {
        let schema = json!({"$dynamicAnchor": "dyn"});
        let mut iter = anchors_202012(Draft::Draft202012, &schema);
        let a = iter.next().unwrap();
        assert_eq!(a.name(), "dyn");
        assert!(matches!(a, Anchor::Dynamic { .. }));
        assert!(iter.next().is_none());
    }

    #[test]
    fn anchor_202012_none() {
        let schema = json!({"type": "string"});
        let mut iter = anchors_202012(Draft::Draft202012, &schema);
        assert!(iter.next().is_none());
    }

    #[test]
    fn anchor_201909() {
        let schema = json!({"$anchor": "myAnchor"});
        let mut iter = anchors_201909(Draft::Draft201909, &schema);
        let a = iter.next().unwrap();
        assert_eq!(a.name(), "myAnchor");
        assert!(iter.next().is_none());
    }

    #[test]
    fn legacy_anchor_dollar_id() {
        let schema = json!({"$id": "#legacyAnchor"});
        let mut iter = legacy_anchor_in_dollar_id(Draft::Draft7, &schema);
        let a = iter.next().unwrap();
        assert_eq!(a.name(), "legacyAnchor");
        assert!(iter.next().is_none());
    }

    #[test]
    fn legacy_anchor_id() {
        let schema = json!({"id": "#d4anchor"});
        let mut iter = legacy_anchor_in_id(Draft::Draft4, &schema);
        let a = iter.next().unwrap();
        assert_eq!(a.name(), "d4anchor");
        assert!(iter.next().is_none());
    }

    #[test]
    fn non_object_returns_empty() {
        let schema = json!(true);
        assert!(anchors_202012(Draft::Draft202012, &schema).next().is_none());
    }
}
