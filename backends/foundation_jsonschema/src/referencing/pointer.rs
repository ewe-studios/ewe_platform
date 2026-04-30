//! JSON Pointer (RFC 6901) parsing and traversal.
//!
//! WHY: JSON Schema `$ref` fragments that start with "/" are JSON Pointers
//! pointing to a location within a schema document. The referencing engine
//! must traverse these pointers to find the target sub-schema.
//!
//! WHAT: `resolve_pointer()` traverses a `serde_json::Value` tree following
//! a JSON Pointer string. `unescape_segment()` handles `~0`/`~1` escaping.
//!
//! HOW: Splits the pointer on "/" delimiters, unescapes each segment, and
//! walks the JSON tree. Array indices are parsed from segments.

use alloc::borrow::Cow;
use alloc::string::String;

use serde_json::Value;

/// JSON Pointer error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointerError {
    /// The pointer string that failed.
    pub pointer: String,
    /// A human-readable description of the failure.
    pub reason: String,
}

impl core::fmt::Display for PointerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "pointer '{}': {}", self.pointer, self.reason)
    }
}

/// Traverse a JSON value using a JSON Pointer string (RFC 6901).
///
/// WHY: `$ref` fragments like `#/definitions/Address` must be resolved
/// to the sub-value at that path within the document.
///
/// WHAT: Returns a reference to the sub-value at the pointer location.
///
/// HOW: Empty string means root document. Otherwise splits on "/" (skipping
/// the leading "/"), unescapes each segment, and navigates objects/arrays.
///
/// # Errors
///
/// Returns `PointerError` if the pointer doesn't start with "/" (when non-empty),
/// or if a segment doesn't exist in the target value.
pub fn resolve_pointer<'a>(document: &'a Value, pointer: &str) -> Result<&'a Value, PointerError> {
    if pointer.is_empty() {
        return Ok(document);
    }
    if !pointer.starts_with('/') {
        return Err(PointerError {
            pointer: pointer.into(),
            reason: "must start with '/'".into(),
        });
    }

    let mut current = document;
    let original_pointer = pointer;

    for segment in pointer[1..].split('/') {
        let unescaped = unescape_segment(segment);
        let decoded = percent_decode_segment(&unescaped).map_err(|e| PointerError {
            pointer: original_pointer.into(),
            reason: e.reason,
        })?;

        current = match current {
            Value::Object(map) => map.get(decoded.as_ref()).ok_or_else(|| PointerError {
                pointer: original_pointer.into(),
                reason: alloc::format!("key '{decoded}' not found"),
            })?,
            Value::Array(arr) => {
                let idx = parse_index(&decoded).ok_or_else(|| PointerError {
                    pointer: original_pointer.into(),
                    reason: alloc::format!("invalid array index '{decoded}'"),
                })?;
                arr.get(idx).ok_or_else(|| PointerError {
                    pointer: original_pointer.into(),
                    reason: alloc::format!("index {} out of bounds (len {})", idx, arr.len()),
                })?
            }
            _ => {
                return Err(PointerError {
                    pointer: original_pointer.into(),
                    reason: alloc::format!("cannot traverse into {}", value_type_name(current)),
                });
            }
        };
    }

    Ok(current)
}

/// Look up a value by a JSON Pointer (returns None instead of error).
///
/// WHY: Some callers (registry build) only need an Option without error context.
pub fn pointer<'a>(document: &'a Value, pointer_str: &str) -> Option<&'a Value> {
    if pointer_str.is_empty() {
        return Some(document);
    }
    if !pointer_str.starts_with('/') {
        return None;
    }
    pointer_str[1..]
        .split('/')
        .map(unescape_segment)
        .try_fold(document, |target, token| match target {
            Value::Object(map) => map.get(&*token),
            Value::Array(list) => parse_index(&token).and_then(|x| list.get(x)),
            _ => None,
        })
}

/// Unescape a JSON Pointer segment: `~1` → `/`, `~0` → `~`.
///
/// WHY: RFC 6901 requires these escape sequences because "/" and "~" have
/// special meaning in JSON Pointer syntax.
///
/// HOW: Single-pass scan for '~', replacing `~1` with '/' and `~0` with '~'.
/// Returns a borrowed `Cow` when no escaping is needed.
#[must_use]
pub fn unescape_segment(mut segment: &str) -> Cow<'_, str> {
    let Some(mut tilde_idx) = segment.find('~') else {
        return Cow::Borrowed(segment);
    };

    let mut buffer = String::with_capacity(segment.len());
    loop {
        let (before, after) = segment.split_at(tilde_idx);
        buffer.push_str(before);
        segment = &after[1..];
        let next_char_size = match segment.chars().next() {
            Some('1') => {
                buffer.push('/');
                1
            }
            Some('0') => {
                buffer.push('~');
                1
            }
            Some(next) => {
                buffer.push('~');
                buffer.push(next);
                next.len_utf8()
            }
            None => {
                buffer.push('~');
                break;
            }
        };
        segment = &segment[next_char_size..];
        let Some(next_tilde_idx) = segment.find('~') else {
            buffer.push_str(segment);
            break;
        };
        tilde_idx = next_tilde_idx;
    }
    Cow::Owned(buffer)
}

/// Percent-decode a segment (for URI fragment JSON Pointers).
fn percent_decode_segment(segment: &str) -> Result<Cow<'_, str>, PointerError> {
    if !segment.contains('%') {
        return Ok(Cow::Borrowed(segment));
    }

    let bytes: alloc::vec::Vec<u8> = percent_encoding::percent_decode_str(segment).collect();

    String::from_utf8(bytes)
        .map(Cow::Owned)
        .map_err(|_| PointerError {
            pointer: segment.into(),
            reason: "invalid percent-encoded UTF-8".into(),
        })
}

/// Parse an array index, rejecting leading zeros and '+'.
fn parse_index(s: &str) -> Option<usize> {
    if s.starts_with('+') || (s.starts_with('0') && s.len() != 1) {
        return None;
    }
    s.parse().ok()
}

fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolve_root() {
        let doc = json!({"a": 1});
        assert_eq!(resolve_pointer(&doc, "").unwrap(), &doc);
    }

    #[test]
    fn resolve_object_key() {
        let doc = json!({"foo": {"bar": 42}});
        assert_eq!(resolve_pointer(&doc, "/foo/bar").unwrap(), &json!(42));
    }

    #[test]
    fn resolve_array_index() {
        let doc = json!({"items": [1, 2, 3]});
        assert_eq!(resolve_pointer(&doc, "/items/1").unwrap(), &json!(2));
    }

    #[test]
    fn resolve_escaped_tilde() {
        let doc = json!({"a~b": 1});
        assert_eq!(resolve_pointer(&doc, "/a~0b").unwrap(), &json!(1));
    }

    #[test]
    fn resolve_escaped_slash() {
        let doc = json!({"a/b": 1});
        assert_eq!(resolve_pointer(&doc, "/a~1b").unwrap(), &json!(1));
    }

    #[test]
    fn resolve_missing_key() {
        let doc = json!({"foo": 1});
        let err = resolve_pointer(&doc, "/bar").unwrap_err();
        assert!(err.reason.contains("key 'bar' not found"));
    }

    #[test]
    fn resolve_index_out_of_bounds() {
        let doc = json!({"items": [1]});
        let err = resolve_pointer(&doc, "/items/5").unwrap_err();
        assert!(err.reason.contains("out of bounds"));
    }

    #[test]
    fn resolve_no_leading_slash() {
        let doc = json!({"a": 1});
        let err = resolve_pointer(&doc, "a").unwrap_err();
        assert!(err.reason.contains("must start with '/'"));
    }

    #[test]
    fn unescape_no_tilde() {
        assert_eq!(unescape_segment("abc"), "abc");
    }

    #[test]
    fn unescape_tilde_zero() {
        assert_eq!(unescape_segment("a~0b"), "a~b");
    }

    #[test]
    fn unescape_tilde_one() {
        assert_eq!(unescape_segment("a~1b"), "a/b");
    }

    #[test]
    fn unescape_both() {
        assert_eq!(unescape_segment("~0~1"), "~/");
    }

    #[test]
    fn unescape_trailing_tilde() {
        assert_eq!(unescape_segment("abc~"), "abc~");
    }

    #[test]
    fn parse_index_valid() {
        assert_eq!(parse_index("0"), Some(0));
        assert_eq!(parse_index("42"), Some(42));
    }

    #[test]
    fn parse_index_leading_zero() {
        assert_eq!(parse_index("01"), None);
    }

    #[test]
    fn parse_index_plus() {
        assert_eq!(parse_index("+1"), None);
    }

    #[test]
    fn pointer_fn_returns_none_for_missing() {
        let doc = json!({"a": 1});
        assert!(pointer(&doc, "/b").is_none());
    }

    #[test]
    fn pointer_fn_returns_some_for_existing() {
        let doc = json!({"a": {"b": 2}});
        assert_eq!(pointer(&doc, "/a/b"), Some(&json!(2)));
    }

    #[test]
    fn unescape_equivalence_property() {
        let inputs = &[
            "abc", "a~0b", "a~1b", "~01", "~10", "a~0~1b", "~", "~~", "~~~~~", "~2", "a~c",
            "~0~1~", "", "a/d", "a~01b",
        ];
        for input in inputs {
            let unescaped = unescape_segment(input);
            let double_replaced = input.replace("~1", "/").replace("~0", "~");
            assert_eq!(&*unescaped, &double_replaced, "Failed for: {input}");
        }
    }
}
