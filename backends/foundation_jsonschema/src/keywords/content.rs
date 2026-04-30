//! `contentEncoding` / `contentMediaType` — validates string encoding and MIME type.
//!
//! WHY: JSON Schema 2019-09+ defines content keywords for strings that encode
//! binary data. These validate that a string value is properly encoded
//! and/or represents valid structured data.
//!
//! When both `contentEncoding` and `contentMediaType` are present on the same
//! schema object, the encoding is decoded first, then the media type is checked
//! against the decoded result (per JSON Schema spec).

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use serde_json::Value;

use crate::error::{ErrorIterator, ValidationError, ValidationErrorBuilder, ValidationErrorKind};
use crate::paths::{LazyLocation, Location};

use super::{Validate, ValidationContext};

/// Supported content encodings with decode capability.
pub enum ContentEncoding {
    Base64,
    QuotedPrintable,
    /// Unknown encoding — annotation only, no decode possible.
    Unknown(String),
}

impl ContentEncoding {
    /// Parse an encoding name into a typed variant.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "base64" => Self::Base64,
            "quoted-printable" => Self::QuotedPrintable,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Decode a string value using this encoding.
    /// Returns `None` if the encoding is unknown or decoding fails.
    pub fn decode(&self, s: &str) -> Option<Vec<u8>> {
        match self {
            Self::Base64 => decode_base64(s),
            Self::QuotedPrintable => decode_quoted_printable(s),
            Self::Unknown(_) => None,
        }
    }

    /// Validate-only check (faster than full decode).
    pub fn is_valid(&self, s: &str) -> bool {
        match self {
            Self::Base64 => is_valid_base64(s),
            Self::QuotedPrintable => is_valid_quoted_printable(s),
            Self::Unknown(_) => true,
        }
    }
}

/// Validates that a string value conforms to a content encoding.
///
/// Supported encodings: `base64`, `quoted-printable`.
/// Other encodings are treated as annotations only (per JSON Schema spec).
pub struct ContentEncodingValidator {
    encoding: ContentEncoding,
    schema_path: Location,
}

impl ContentEncodingValidator {
    /// Create a new content encoding validator.
    #[must_use]
    pub fn new(encoding: &str, schema_path: Location) -> Self {
        Self {
            encoding: ContentEncoding::from_name(encoding),
            schema_path,
        }
    }
}

impl Validate for ContentEncodingValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        let Value::String(s) = instance else {
            return true;
        };
        self.encoding.is_valid(s)
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        if self.is_valid(instance, ctx) {
            Ok(())
        } else {
            let encoding_name = match &self.encoding {
                ContentEncoding::Base64 => "base64",
                ContentEncoding::QuotedPrintable => "quoted-printable",
                ContentEncoding::Unknown(name) => name.as_str(),
            };
            Err(
                ValidationErrorBuilder::new(instance_path.materialize(), self.schema_path.clone())
                    .build(ValidationErrorKind::ContentEncoding {
                        encoding: encoding_name.to_string(),
                    }),
            )
        }
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        if self.is_valid(instance, ctx) {
            Box::new(core::iter::empty())
        } else {
            let encoding_name = match &self.encoding {
                ContentEncoding::Base64 => "base64",
                ContentEncoding::QuotedPrintable => "quoted-printable",
                ContentEncoding::Unknown(name) => name.as_str(),
            };
            let err =
                ValidationErrorBuilder::new(instance_path.materialize(), self.schema_path.clone())
                    .build(ValidationErrorKind::ContentEncoding {
                        encoding: encoding_name.to_string(),
                    });
            Box::new(core::iter::once(err))
        }
    }
}

/// Validates `contentMediaType` alone (no encoding present).
///
/// Supported media types: `application/json`.
/// Other media types are treated as annotations only (per JSON Schema spec).
pub struct ContentMediaTypeValidator {
    media_type: String,
    schema_path: Location,
}

impl ContentMediaTypeValidator {
    /// Create a new content media type validator.
    #[must_use]
    pub fn new(media_type: String, schema_path: Location) -> Self {
        Self {
            media_type,
            schema_path,
        }
    }
}

impl Validate for ContentMediaTypeValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        let Value::String(s) = instance else {
            return true;
        };
        match self.media_type.as_str() {
            "application/json" => serde_json::from_str::<Value>(s).is_ok(),
            _ => true,
        }
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        if self.is_valid(instance, ctx) {
            Ok(())
        } else {
            Err(
                ValidationErrorBuilder::new(instance_path.materialize(), self.schema_path.clone())
                    .build(ValidationErrorKind::ContentMediaType {
                        media_type: self.media_type.clone(),
                    }),
            )
        }
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        if self.is_valid(instance, ctx) {
            Box::new(core::iter::empty())
        } else {
            let err =
                ValidationErrorBuilder::new(instance_path.materialize(), self.schema_path.clone())
                    .build(ValidationErrorKind::ContentMediaType {
                        media_type: self.media_type.clone(),
                    });
            Box::new(core::iter::once(err))
        }
    }
}

/// Combined validator: decode via `contentEncoding`, then check `contentMediaType` on decoded result.
///
/// WHY: When both keywords are present, the spec requires decoding first, then
/// validating the decoded bytes as the media type.
pub struct ContentCombinedValidator {
    encoding: ContentEncoding,
    media_type: String,
    encoding_schema_path: Location,
    media_type_schema_path: Location,
}

impl ContentCombinedValidator {
    /// Create a new combined validator.
    #[must_use]
    pub fn new(
        encoding: ContentEncoding,
        media_type: String,
        encoding_schema_path: Location,
        media_type_schema_path: Location,
    ) -> Self {
        Self {
            encoding,
            media_type,
            encoding_schema_path,
            media_type_schema_path,
        }
    }
}

impl Validate for ContentCombinedValidator {
    fn is_valid(&self, instance: &Value, _ctx: &mut ValidationContext) -> bool {
        let Value::String(s) = instance else {
            return true;
        };
        // For unknown encodings, skip decode and only check media type on raw string
        if matches!(self.encoding, ContentEncoding::Unknown(_)) {
            return self.is_media_type_valid(s);
        }
        // Decode, then check media type on decoded result
        let Some(decoded) = self.encoding.decode(s) else {
            return false;
        };
        let Ok(decoded_str) = core::str::from_utf8(&decoded) else {
            return false;
        };
        self.is_media_type_valid(decoded_str)
    }

    fn validate(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        _ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        let Value::String(s) = instance else {
            return Ok(());
        };

        if matches!(self.encoding, ContentEncoding::Unknown(_)) {
            if self.is_media_type_valid(s) {
                return Ok(());
            }
            return Err(ValidationErrorBuilder::new(
                instance_path.materialize(),
                self.media_type_schema_path.clone(),
            )
            .build(ValidationErrorKind::ContentMediaType {
                media_type: self.media_type.clone(),
            }));
        }

        let Some(decoded) = self.encoding.decode(s) else {
            return Err(ValidationErrorBuilder::new(
                instance_path.materialize(),
                self.encoding_schema_path.clone(),
            )
            .build(ValidationErrorKind::ContentEncoding {
                encoding: match &self.encoding {
                    ContentEncoding::Base64 => "base64",
                    ContentEncoding::QuotedPrintable => "quoted-printable",
                    ContentEncoding::Unknown(_) => unreachable!(),
                }
                .to_string(),
            }));
        };

        let Ok(decoded_str) = core::str::from_utf8(&decoded) else {
            return Err(ValidationErrorBuilder::new(
                instance_path.materialize(),
                self.encoding_schema_path.clone(),
            )
            .build(ValidationErrorKind::ContentEncoding {
                encoding: "base64".to_string(),
            }));
        };

        if self.is_media_type_valid(decoded_str) {
            Ok(())
        } else {
            Err(ValidationErrorBuilder::new(
                instance_path.materialize(),
                self.media_type_schema_path.clone(),
            )
            .build(ValidationErrorKind::ContentMediaType {
                media_type: self.media_type.clone(),
            }))
        }
    }

    fn iter_errors(
        &self,
        instance: &Value,
        instance_path: &LazyLocation<'_>,
        ctx: &mut ValidationContext,
    ) -> ErrorIterator {
        match self.validate(instance, instance_path, ctx) {
            Ok(()) => Box::new(core::iter::empty()),
            Err(e) => Box::new(core::iter::once(e)),
        }
    }
}

impl ContentCombinedValidator {
    fn is_media_type_valid(&self, s: &str) -> bool {
        match self.media_type.as_str() {
            "application/json" => serde_json::from_str::<Value>(s).is_ok(),
            _ => true,
        }
    }
}

/// Supported media types.
#[allow(dead_code)]
pub enum ContentMediaType {
    ApplicationJson,
    /// Unknown media type — annotation only.
    Unknown(String),
}

impl ContentMediaType {
    /// Parse a media type name.
    #[allow(dead_code)]
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "application/json" => Self::ApplicationJson,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Check if a string value conforms to this media type.
    #[allow(dead_code)]
    pub fn is_valid(&self, s: &str) -> bool {
        match self {
            Self::ApplicationJson => serde_json::from_str::<Value>(s).is_ok(),
            Self::Unknown(_) => true,
        }
    }
}

/// Check if a string is valid base64 (RFC 4648).
fn is_valid_base64(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    if !s.len().is_multiple_of(4) {
        return false;
    }
    let bytes = s.as_bytes();
    let len = bytes.len();
    // Count trailing '=' (max 2)
    let pad = if bytes[len - 1] == b'=' {
        if len >= 2 && bytes[len - 2] == b'=' {
            2
        } else {
            1
        }
    } else {
        0
    };
    // Validate data portion (no '=' allowed in middle)
    let data_end = len - pad;
    bytes[..data_end]
        .iter()
        .all(|&b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/'))
}

/// Check if a string is valid quoted-printable encoding.
fn is_valid_quoted_printable(s: &str) -> bool {
    // Quoted-printable: =XX hex sequences or printable ASCII (33-126, except '=')
    // Also allows soft line breaks: =\n or =\r\n
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '=' {
            // Soft line break
            if let Some(&'\r') = chars.peek() {
                chars.next();
                if let Some(&'\n') = chars.peek() {
                    chars.next();
                } else {
                    return false; // CR must be followed by LF
                }
            } else if let Some(&'\n') = chars.peek() {
                chars.next();
            } else {
                // Hex pair
                let hex1 = chars.next();
                let hex2 = chars.next();
                match (hex1, hex2) {
                    (Some(h1), Some(h2)) if h1.is_ascii_hexdigit() && h2.is_ascii_hexdigit() => {}
                    _ => return false,
                }
            }
        } else if !('!'..='~').contains(&c) {
            // Not a printable ASCII character — allow space and tab
            if c != ' ' && c != '\t' {
                return false;
            }
        }
    }
    true
}

/// Decode a base64-encoded string into bytes.
#[allow(
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::unnecessary_cast
)]
fn decode_base64(s: &str) -> Option<Vec<u8>> {
    if s.is_empty() {
        return Some(Vec::new());
    }
    if !s.len().is_multiple_of(4) {
        return None;
    }
    let bytes = s.as_bytes();
    let len = bytes.len();
    let pad = if bytes[len - 1] == b'=' {
        if len >= 2 && bytes[len - 2] == b'=' {
            2
        } else {
            1
        }
    } else {
        0
    };
    let data_end = len - pad;
    for &b in &bytes[..data_end] {
        if !matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/') {
            return None;
        }
    }
    let mut result = Vec::with_capacity((data_end / 4) * 3);
    let mut idx = 0;
    while idx + 4 <= data_end {
        let w = b64_val(bytes[idx])? << 18
            | b64_val(bytes[idx + 1])? << 12
            | b64_val(bytes[idx + 2])? << 6
            | b64_val(bytes[idx + 3])?;
        result.push((w >> 16) as u8);
        result.push((w >> 8) as u8);
        result.push(w as u8);
        idx += 4;
    }
    // Handle final group with padding
    if data_end < len {
        let data_count = 4 - (len - data_end);
        if idx + data_count <= data_end {
            let a = b64_val(bytes[idx])?;
            let b = if data_count > 1 {
                b64_val(bytes[idx + 1])?
            } else {
                0
            };
            let w = a << 18 | b << 12;
            result.push((w >> 16) as u8);
            // 3 data chars → 2 bytes; 2 data chars → 1 byte
            if data_count > 2 {
                result.push((w >> 8) as u8);
            }
        }
    }
    Some(result)
}

#[allow(clippy::cast_lossless)]
fn b64_val(c: u8) -> Option<u32> {
    Some(match c {
        b'A'..=b'Z' => u32::from(c - b'A'),
        b'a'..=b'z' => u32::from(c - b'a' + 26),
        b'0'..=b'9' => u32::from(c - b'0' + 52),
        b'+' => 62,
        b'/' => 63,
        b'=' => 0,
        _ => return None,
    })
}

/// Decode a quoted-printable-encoded string into bytes.
#[allow(clippy::uninlined_format_args)]
fn decode_quoted_printable(s: &str) -> Option<Vec<u8>> {
    let mut result = Vec::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '=' {
            if let Some(&'\r') = chars.peek() {
                chars.next();
                if let Some(&'\n') = chars.peek() {
                    chars.next();
                } else {
                    return None;
                }
            } else if let Some(&'\n') = chars.peek() {
                chars.next();
            } else {
                let h1 = chars.next()?;
                let h2 = chars.next()?;
                let byte = u8::from_str_radix(&alloc::format!("{h1}{h2}"), 16).ok()?;
                result.push(byte);
            }
        } else if ('!'..='~').contains(&c) || c == ' ' || c == '\t' {
            result.push(c as u8);
        } else {
            return None;
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> ValidationContext {
        ValidationContext::new()
    }

    // ── base64 validation ──────────────────────────────────────

    #[test]
    fn valid_base64_standard() {
        assert!(is_valid_base64("SGVsbG8="));
        assert!(is_valid_base64("SGVsbG8gV29ybGQ="));
        assert!(is_valid_base64("AAAA"));
    }

    #[test]
    fn valid_base64_empty() {
        assert!(is_valid_base64(""));
    }

    #[test]
    fn valid_base64_padding() {
        assert!(is_valid_base64("YWI=")); // "ab" with 1 pad
        assert!(is_valid_base64("YQ==")); // "a" with 2 pads
    }

    #[test]
    fn invalid_base64_wrong_length() {
        assert!(!is_valid_base64("SGVsbG8")); // 7 chars, not multiple of 4
        assert!(!is_valid_base64("A"));
    }

    #[test]
    fn valid_base64_lowercase() {
        // RFC 4648 base64 uses A-Z, a-z, 0-9, +, /
        assert!(is_valid_base64("aaaa")); // lowercase is valid base64
    }

    #[test]
    fn invalid_base64_bad_padding() {
        assert!(!is_valid_base64("A===")); // 3 pads not allowed
    }

    #[test]
    fn invalid_base64_bad_chars() {
        assert!(!is_valid_base64("SGVs!G8=")); // ! not in base64 alphabet
        assert!(!is_valid_base64("SGVs bG8=")); // space not allowed
    }

    // ── quoted-printable validation ────────────────────────────

    #[test]
    fn valid_qp_simple() {
        assert!(is_valid_quoted_printable("Hello=20World"));
        assert!(is_valid_quoted_printable("=48=65=6C=6C=6F"));
    }

    #[test]
    fn valid_qp_soft_linebreak() {
        assert!(is_valid_quoted_printable("Hello=\r\nWorld"));
        assert!(is_valid_quoted_printable("Hello=\nWorld"));
    }

    #[test]
    fn invalid_qp_incomplete_hex() {
        assert!(!is_valid_quoted_printable("Hello=2"));
        assert!(!is_valid_quoted_printable("Hello="));
    }

    // ── content encoding validator ─────────────────────────────

    #[test]
    fn content_encoding_base64_valid() {
        let v = ContentEncodingValidator::new("base64", Location::new());
        assert!(v.is_valid(&json!("SGVsbG8="), &mut ctx()));
    }

    #[test]
    fn content_encoding_base64_invalid() {
        let v = ContentEncodingValidator::new("base64", Location::new());
        assert!(!v.is_valid(&json!("not-valid!!!"), &mut ctx()));
    }

    #[test]
    fn content_encoding_non_string_always_valid() {
        let v = ContentEncodingValidator::new("base64", Location::new());
        assert!(v.is_valid(&json!(42), &mut ctx()));
    }

    #[test]
    fn content_encoding_unknown_passes() {
        // Unknown encodings are annotation-only
        let v = ContentEncodingValidator::new("rot13", Location::new());
        assert!(v.is_valid(&json!("anything"), &mut ctx()));
    }

    // ── content media type validator ───────────────────────────

    #[test]
    fn content_media_type_json_valid() {
        let v = ContentMediaTypeValidator::new("application/json".into(), Location::new());
        assert!(v.is_valid(&json!({"key": "value"}), &mut ctx()));
    }

    #[test]
    fn content_media_type_json_invalid() {
        let v = ContentMediaTypeValidator::new("application/json".into(), Location::new());
        assert!(!v.is_valid(&json!("not json"), &mut ctx()));
    }

    #[test]
    fn content_media_type_unknown_passes() {
        let v = ContentMediaTypeValidator::new("text/plain".into(), Location::new());
        assert!(v.is_valid(&json!("anything"), &mut ctx()));
    }

    // ── combined encoding + media type ─────────────────────────

    #[test]
    fn combined_base64_json_valid() {
        // base64 encoded: {"answer":42}
        let encoded = "eyJhbnN3ZXIiOjQyfQ==";
        let v = ContentCombinedValidator::new(
            ContentEncoding::Base64,
            "application/json".into(),
            Location::new(),
            Location::new(),
        );
        assert!(v.is_valid(&json!(encoded), &mut ctx()));
    }

    #[test]
    fn combined_base64_json_invalid() {
        // base64 encoded: "not json" -> decodes but isn't valid JSON
        let encoded = "bm90IGpzb24=";
        let v = ContentCombinedValidator::new(
            ContentEncoding::Base64,
            "application/json".into(),
            Location::new(),
            Location::new(),
        );
        assert!(!v.is_valid(&json!(encoded), &mut ctx()));
    }

    #[test]
    fn combined_bad_encoding() {
        let v = ContentCombinedValidator::new(
            ContentEncoding::Base64,
            "application/json".into(),
            Location::new(),
            Location::new(),
        );
        assert!(!v.is_valid(&json!("not-base64!"), &mut ctx()));
    }
}
