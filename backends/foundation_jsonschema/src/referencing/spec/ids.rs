//! Schema ID extraction functions used across drafts.

use serde_json::Value;

/// Extract `$id` from a schema (Drafts 2019-09 and 2020-12).
pub fn dollar_id(contents: &Value) -> Option<&str> {
    contents
        .as_object()
        .and_then(|obj| obj.get("$id"))
        .and_then(Value::as_str)
}

/// Extract `$id` from a schema (Drafts 6 and 7).
///
/// In these drafts, `$ref` suppresses `$id`, and `$id` values starting
/// with '#' are anchors rather than resource IDs.
pub fn legacy_dollar_id(contents: &Value) -> Option<&str> {
    let object = contents.as_object()?;
    if object.contains_key("$ref") {
        return None;
    }
    if let Some(id) = object.get("$id").and_then(Value::as_str) {
        if !id.starts_with('#') {
            return Some(id);
        }
    }
    None
}

/// Extract `id` from a schema (Draft 4).
///
/// Draft 4 uses `id` (not `$id`). `$ref` suppresses `id`, and `id` values
/// starting with '#' are anchors.
pub fn legacy_id(contents: &Value) -> Option<&str> {
    let object = contents.as_object()?;
    if object.contains_key("$ref") {
        return None;
    }
    if let Some(id) = object.get("id").and_then(Value::as_str) {
        if !id.starts_with('#') {
            return Some(id);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dollar_id_extracts() {
        let v = json!({"$id": "http://example.com"});
        assert_eq!(dollar_id(&v), Some("http://example.com"));
    }

    #[test]
    fn dollar_id_missing() {
        let v = json!({"type": "string"});
        assert_eq!(dollar_id(&v), None);
    }

    #[test]
    fn legacy_dollar_id_extracts() {
        let v = json!({"$id": "http://example.com"});
        assert_eq!(legacy_dollar_id(&v), Some("http://example.com"));
    }

    #[test]
    fn legacy_dollar_id_suppressed_by_ref() {
        let v = json!({"$id": "http://example.com", "$ref": "#/defs/Foo"});
        assert_eq!(legacy_dollar_id(&v), None);
    }

    #[test]
    fn legacy_dollar_id_ignores_fragment() {
        let v = json!({"$id": "#anchor"});
        assert_eq!(legacy_dollar_id(&v), None);
    }

    #[test]
    fn legacy_id_extracts() {
        let v = json!({"id": "http://example.com"});
        assert_eq!(legacy_id(&v), Some("http://example.com"));
    }

    #[test]
    fn legacy_id_suppressed_by_ref() {
        let v = json!({"id": "http://example.com", "$ref": "#"});
        assert_eq!(legacy_id(&v), None);
    }

    #[test]
    fn legacy_id_ignores_fragment() {
        let v = json!({"id": "#anchor"});
        assert_eq!(legacy_id(&v), None);
    }
}
