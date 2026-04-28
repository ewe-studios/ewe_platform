//! Draft 2020-12 specific sub-resource traversal.

use alloc::vec::Vec;
use serde_json::{Map, Value};

use crate::draft::Draft;

pub fn collect_subresources<'a>(
    schema: &'a Map<String, Value>,
    draft: Draft,
    items: &mut Vec<&'a Value>,
) {
    for (key, value) in schema {
        match key.as_str() {
            "additionalProperties"
            | "contains"
            | "contentSchema"
            | "else"
            | "if"
            | "items"
            | "not"
            | "propertyNames"
            | "then"
            | "unevaluatedItems"
            | "unevaluatedProperties" => {
                items.push(value);
            }
            "allOf" | "anyOf" | "oneOf" | "prefixItems" => {
                if let Some(arr) = value.as_array() {
                    items.extend(arr.iter());
                }
            }
            "$defs" | "definitions" | "dependentSchemas" | "patternProperties" | "properties" => {
                if let Some(obj) = value.as_object() {
                    items.extend(obj.values());
                }
            }
            _ => {}
        }
    }
}
