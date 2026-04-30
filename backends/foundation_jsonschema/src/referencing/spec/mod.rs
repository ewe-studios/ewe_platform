//! Draft-specific schema analysis dispatch.
//!
//! WHY: Each JSON Schema draft has different rules for ID extraction, anchor
//! extraction, and which keywords contain sub-schemas. The spec module
//! dispatches to the correct draft-specific logic.
//!
//! WHAT: `ObjectAnalysis` aggregates what we learn from scanning one schema
//! object. `analyze_object()` dispatches to the right draft-specific scanner.
//!
//! HOW: Single pass over the schema object's keys, extracting `$id`/`id`,
//! `$anchor`/`$dynamicAnchor`, `$ref`, and `$schema`.

pub mod draft201909;
pub mod draft202012;
pub mod draft4;
pub mod draft6;
pub mod draft7;
pub mod ids;

use serde_json::{Map, Value};

use super::anchor::AnchorIter;
use crate::draft::Draft;

/// Metadata extracted from one schema object during registry crawling.
pub struct ObjectAnalysis<'a> {
    /// The schema's ID (`$id` or `id`), if any.
    pub id: Option<&'a str>,
    /// Whether the schema declares any anchor(s).
    pub has_anchor: bool,
    /// The `$ref` value, if any.
    pub dollar_ref: Option<&'a str>,
    /// The `$schema` meta-schema URI, if any.
    pub meta_schema: Option<&'a str>,
}

/// Analyze a schema object for a modern draft (2019-09 / 2020-12).
///
/// `$anchor` is always checked. `$dynamicAnchor` is checked only when
/// `dynamic_anchor` is true (Draft 2020-12).
pub fn analyze_object_modern(
    schema: &Map<String, Value>,
    dynamic_anchor: bool,
) -> ObjectAnalysis<'_> {
    let mut id = None;
    let mut has_anchor = false;
    let mut dollar_ref = None;
    let mut meta_schema = None;

    for (key, value) in schema {
        match key.as_str() {
            "$id" => id = value.as_str(),
            "$anchor" => has_anchor |= value.as_str().is_some(),
            "$dynamicAnchor" if dynamic_anchor => has_anchor |= value.as_str().is_some(),
            "$ref" => dollar_ref = value.as_str(),
            "$schema" => meta_schema = value.as_str(),
            _ => {}
        }
    }

    ObjectAnalysis {
        id,
        has_anchor,
        dollar_ref,
        meta_schema,
    }
}

/// Analyze a schema object for a classic draft (4, 6, 7).
///
/// `id_key` is `"id"` for Draft 4 or `"$id"` for Drafts 6/7. In these
/// drafts, `$ref` suppresses the ID, and IDs starting with `#` are anchors.
pub fn analyze_object_classic<'a>(
    schema: &'a Map<String, Value>,
    id_key: &str,
) -> ObjectAnalysis<'a> {
    let mut raw_id = None;
    let mut dollar_ref = None;
    let mut meta_schema = None;

    for (key, value) in schema {
        let k = key.as_str();
        if k == id_key {
            raw_id = value.as_str();
        } else if k == "$ref" {
            dollar_ref = value.as_str();
        } else if k == "$schema" {
            meta_schema = value.as_str();
        }
    }

    let has_anchor = raw_id.is_some_and(|id| id.starts_with('#'));
    let id = match raw_id {
        Some(id) if !has_anchor && dollar_ref.is_none() => Some(id),
        _ => None,
    };

    ObjectAnalysis {
        id,
        has_anchor,
        dollar_ref,
        meta_schema,
    }
}

/// Dispatch `analyze_object` to the correct draft-specific logic.
pub fn analyze_object(draft: Draft, schema: &Map<String, Value>) -> ObjectAnalysis<'_> {
    match draft {
        Draft::Draft4 => analyze_object_classic(schema, "id"),
        Draft::Draft6 | Draft::Draft7 => analyze_object_classic(schema, "$id"),
        Draft::Draft201909 => analyze_object_modern(schema, false),
        Draft::Draft202012 => analyze_object_modern(schema, true),
    }
}

/// Extract the schema ID per draft rules.
pub fn id_of(draft: Draft, contents: &Value) -> Option<&str> {
    match draft {
        Draft::Draft4 => ids::legacy_id(contents),
        Draft::Draft6 | Draft::Draft7 => ids::legacy_dollar_id(contents),
        Draft::Draft201909 | Draft::Draft202012 => ids::dollar_id(contents),
    }
}

/// Extract anchors per draft rules.
pub fn anchors_of(draft: Draft, contents: &Value) -> AnchorIter<'_> {
    use super::anchor;
    match draft {
        Draft::Draft4 => anchor::legacy_anchor_in_id(draft, contents),
        Draft::Draft6 | Draft::Draft7 => anchor::legacy_anchor_in_dollar_id(draft, contents),
        Draft::Draft201909 => anchor::anchors_201909(draft, contents),
        Draft::Draft202012 => anchor::anchors_202012(draft, contents),
    }
}

/// Iterate over child sub-schemas that should be traversed during registry building.
///
/// Each draft defines which keywords contain sub-schemas.
pub fn subresources_of(draft: Draft, contents: &Value) -> SubresourceIter<'_> {
    let Some(object) = contents.as_object() else {
        return SubresourceIter {
            items: alloc::vec::Vec::new(),
        };
    };

    let mut items = alloc::vec::Vec::new();

    match draft {
        Draft::Draft4 => draft4::collect_subresources(object, draft, &mut items),
        Draft::Draft6 => draft6::collect_subresources(object, draft, &mut items),
        Draft::Draft7 => draft7::collect_subresources(object, draft, &mut items),
        Draft::Draft201909 => draft201909::collect_subresources(object, draft, &mut items),
        Draft::Draft202012 => draft202012::collect_subresources(object, draft, &mut items),
    }

    SubresourceIter { items }
}

/// Iterator over child sub-schemas.
pub struct SubresourceIter<'a> {
    items: alloc::vec::Vec<&'a Value>,
}

impl<'a> Iterator for SubresourceIter<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<Self::Item> {
        self.items.pop()
    }
}

use alloc::vec::Vec;

/// Collect single-valued keyword children.
fn collect_single<'a>(object: &'a Map<String, Value>, key: &str, items: &mut Vec<&'a Value>) {
    if let Some(value) = object.get(key) {
        items.push(value);
    }
}

/// Collect array-valued keyword children.
fn collect_array<'a>(object: &'a Map<String, Value>, key: &str, items: &mut Vec<&'a Value>) {
    if let Some(Value::Array(arr)) = object.get(key) {
        items.extend(arr.iter());
    }
}

/// Collect object-valued keyword children (all values).
fn collect_object_values<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    items: &mut Vec<&'a Value>,
) {
    if let Some(Value::Object(obj)) = object.get(key) {
        items.extend(obj.values());
    }
}

/// Collect object-valued keyword children (only object values).
fn collect_object_values_filtered<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    items: &mut Vec<&'a Value>,
) {
    if let Some(Value::Object(obj)) = object.get(key) {
        items.extend(obj.values().filter(|v| v.is_object()));
    }
}
