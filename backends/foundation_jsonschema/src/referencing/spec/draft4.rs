//! Draft 4 specific sub-resource traversal.

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
            "additionalItems" | "additionalProperties" if value.is_object() => {
                items.push(value);
            }
            "not" => {
                items.push(value);
            }
            "allOf" | "anyOf" | "oneOf" => {
                if let Some(arr) = value.as_array() {
                    items.extend(arr.iter());
                }
            }
            "items" => match value {
                Value::Array(arr) => items.extend(arr.iter()),
                _ => items.push(value),
            },
            "definitions" | "patternProperties" | "properties" => {
                if let Some(obj) = value.as_object() {
                    items.extend(obj.values());
                }
            }
            "dependencies" => {
                if let Some(obj) = value.as_object() {
                    items.extend(obj.values().filter(|v| v.is_object()));
                }
            }
            _ => {}
        }
    }
}
