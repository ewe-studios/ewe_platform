//! WHY: A human-readable format for debugging, SSE `text/event-stream-json`, and
//! servers that prefer JSON over binary.
//!
//! WHAT: [`JsonEncoder`] — a `DomOp` batch as a JSON array of FLAT objects whose
//! field names are the Arrow column names (feature 01 spec section 4.2):
//!
//! ```json
//! [
//!   { "op_id": 0, "node_id": 5, "operation": 2,
//!     "attribute": null, "value": null, "text_val": "hello" }
//! ]
//! ```
//!
//! HOW: Encoding walks the shared [`Row`] mapping and prints each row; decoding
//! parses with the in-crate `no_std` JSON subset parser, rebuilds rows, and runs
//! the same `Row::into_op` the Arrow decoder uses — one mapping, every format.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::encoder::{
    DecodeError, DecodeResult, ProtocolEncoder, Row, PROTOCOL_JSON, PROTOCOL_VERSION,
};
use crate::DomOp;

/// The flat-row JSON [`ProtocolEncoder`] (protocol byte 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JsonEncoder;

impl ProtocolEncoder<Vec<DomOp>> for JsonEncoder {
    fn protocol_byte(&self) -> u8 {
        PROTOCOL_JSON
    }

    fn version(&self) -> u8 {
        PROTOCOL_VERSION
    }

    fn encode(&self, data: Vec<DomOp>) -> Vec<u8> {
        let mut s = String::from("[");
        for (i, op) in data.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            write_row_object(&mut s, i, &Row::from_op(op));
        }
        s.push(']');
        s.into_bytes()
    }

    fn decode(&self, payload: &[u8]) -> DecodeResult<Vec<DomOp>> {
        let text = core::str::from_utf8(payload).map_err(|_| DecodeError::JsonParseError {
            detail: String::from("payload is not UTF-8"),
        })?;
        let value = json::parse(text)?;
        let json::Value::Array(array) = value else {
            return Err(DecodeError::JsonParseError {
                detail: String::from("top-level value is not an array"),
            });
        };
        let mut ops = Vec::with_capacity(array.len());
        for (i, item) in array.into_iter().enumerate() {
            ops.push(row_from_json(item, i)?.into_op(i)?);
        }
        Ok(ops)
    }
}

/// Print one flat row object with the Arrow column field names.
fn write_row_object(out: &mut String, op_id: usize, row: &Row) {
    out.push_str("{\"op_id\":");
    out.push_str(&op_id.to_string());
    out.push_str(",\"node_id\":");
    out.push_str(&row.node_id.to_string());
    out.push_str(",\"operation\":");
    out.push_str(&row.operation.to_string());
    for (name, cell) in [
        ("attribute", &row.attribute),
        ("value", &row.value),
        ("text_val", &row.text_val),
    ] {
        out.push_str(",\"");
        out.push_str(name);
        out.push_str("\":");
        match cell {
            Some(s) => json::write_string(out, s),
            None => out.push_str("null"),
        }
    }
    out.push('}');
}

/// Rebuild a [`Row`] from one decoded flat JSON object.
fn row_from_json(value: json::Value, row_index: usize) -> DecodeResult<Row> {
    let json::Value::Object(obj) = value else {
        return Err(DecodeError::JsonParseError {
            detail: format!("row {row_index}: array item is not an object"),
        });
    };

    let find = |key: &str| obj.iter().find(|(k, _)| k == key).map(|(_, v)| v);
    let get_num = |key: &str| -> DecodeResult<u32> {
        match find(key) {
            Some(json::Value::Number(n)) => Ok(*n),
            _ => Err(DecodeError::JsonParseError {
                detail: format!("row {row_index}: missing or non-numeric `{key}`"),
            }),
        }
    };
    let get_nullable_str = |key: &str| -> DecodeResult<Option<String>> {
        match find(key) {
            Some(json::Value::String(s)) => Ok(Some(s.clone())),
            Some(json::Value::Null) | None => Ok(None),
            _ => Err(DecodeError::JsonParseError {
                detail: format!("row {row_index}: `{key}` is neither string nor null"),
            }),
        }
    };

    let operation = get_num("operation")?;
    let operation = u8::try_from(operation).map_err(|_| DecodeError::JsonParseError {
        detail: format!("row {row_index}: `operation` {operation} exceeds u8"),
    })?;

    Ok(Row {
        operation,
        node_id: get_num("node_id")?,
        attribute: get_nullable_str("attribute")?,
        value: get_nullable_str("value")?,
        text_val: get_nullable_str("text_val")?,
    })
}

// ─── Minimal no_std JSON value parser ──────────────────────────────────────────
//
// Scoped to exactly what `JsonEncoder` emits: arrays of flat objects whose values
// are strings, non-negative integers, or null. It correctly handles JSON string
// escapes so round-tripping arbitrary text content works. Not a general-purpose
// parser.
mod json {
    use super::DecodeError;
    use alloc::format;
    use alloc::string::String;
    use alloc::vec::Vec;

    /// A decoded JSON value (subset).
    pub enum Value {
        Null,
        String(String),
        Number(u32),
        Object(Vec<(String, Value)>),
        Array(Vec<Value>),
    }

    /// Escape and quote `s` into `out` per the JSON string grammar.
    pub fn write_string(out: &mut String, s: &str) {
        out.push('"');
        for ch in s.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => {
                    out.push_str("\\u");
                    let code = c as u32;
                    for shift in [12u32, 8, 4, 0] {
                        let digit = (code >> shift) & 0xF;
                        out.push(core::char::from_digit(digit, 16).unwrap_or('0'));
                    }
                }
                c => out.push(c),
            }
        }
        out.push('"');
    }

    /// Parse `text` into a [`Value`].
    ///
    /// # Errors
    /// Returns [`DecodeError::JsonParseError`] with the byte position of the
    /// first offending character.
    pub fn parse(text: &str) -> Result<Value, DecodeError> {
        let mut parser = Parser {
            bytes: text.as_bytes(),
            pos: 0,
        };
        parser.skip_ws();
        let value = parser.value()?;
        parser.skip_ws();
        if parser.pos != parser.bytes.len() {
            return Err(parser.error("trailing characters"));
        }
        Ok(value)
    }

    struct Parser<'a> {
        bytes: &'a [u8],
        pos: usize,
    }

    impl Parser<'_> {
        fn error(&self, what: &str) -> DecodeError {
            DecodeError::JsonParseError {
                detail: format!("{what} at byte {}", self.pos),
            }
        }

        fn peek(&self) -> Option<u8> {
            self.bytes.get(self.pos).copied()
        }

        fn skip_ws(&mut self) {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                self.pos += 1;
            }
        }

        fn expect(&mut self, byte: u8) -> Result<(), DecodeError> {
            if self.peek() == Some(byte) {
                self.pos += 1;
                Ok(())
            } else {
                Err(self.error("unexpected character"))
            }
        }

        fn literal(&mut self, lit: &str) -> bool {
            let end = self.pos + lit.len();
            if self.bytes.get(self.pos..end) == Some(lit.as_bytes()) {
                self.pos = end;
                true
            } else {
                false
            }
        }

        fn value(&mut self) -> Result<Value, DecodeError> {
            match self.peek() {
                Some(b'[') => self.array(),
                Some(b'{') => self.object(),
                Some(b'"') => Ok(Value::String(self.string()?)),
                Some(b'0'..=b'9') => self.number(),
                Some(b'n') if self.literal("null") => Ok(Value::Null),
                _ => Err(self.error("expected a value")),
            }
        }

        fn array(&mut self) -> Result<Value, DecodeError> {
            self.expect(b'[')?;
            let mut items = Vec::new();
            self.skip_ws();
            if self.peek() == Some(b']') {
                self.pos += 1;
                return Ok(Value::Array(items));
            }
            loop {
                self.skip_ws();
                items.push(self.value()?);
                self.skip_ws();
                match self.peek() {
                    Some(b',') => self.pos += 1,
                    Some(b']') => {
                        self.pos += 1;
                        return Ok(Value::Array(items));
                    }
                    _ => return Err(self.error("expected `,` or `]`")),
                }
            }
        }

        fn object(&mut self) -> Result<Value, DecodeError> {
            self.expect(b'{')?;
            let mut fields = Vec::new();
            self.skip_ws();
            if self.peek() == Some(b'}') {
                self.pos += 1;
                return Ok(Value::Object(fields));
            }
            loop {
                self.skip_ws();
                let key = self.string()?;
                self.skip_ws();
                self.expect(b':')?;
                self.skip_ws();
                let value = self.value()?;
                fields.push((key, value));
                self.skip_ws();
                match self.peek() {
                    Some(b',') => self.pos += 1,
                    Some(b'}') => {
                        self.pos += 1;
                        return Ok(Value::Object(fields));
                    }
                    _ => return Err(self.error("expected `,` or `}`")),
                }
            }
        }

        fn number(&mut self) -> Result<Value, DecodeError> {
            let start = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            let text = core::str::from_utf8(&self.bytes[start..self.pos])
                .map_err(|_| self.error("number"))?;
            let n = text.parse::<u32>().map_err(|_| self.error("number range"))?;
            Ok(Value::Number(n))
        }

        fn string(&mut self) -> Result<String, DecodeError> {
            self.expect(b'"')?;
            let mut out = String::new();
            loop {
                let Some(b) = self.peek() else {
                    return Err(self.error("unterminated string"));
                };
                self.pos += 1;
                match b {
                    b'"' => return Ok(out),
                    b'\\' => {
                        let Some(esc) = self.peek() else {
                            return Err(self.error("unterminated escape"));
                        };
                        self.pos += 1;
                        match esc {
                            b'"' => out.push('"'),
                            b'\\' => out.push('\\'),
                            b'/' => out.push('/'),
                            b'n' => out.push('\n'),
                            b'r' => out.push('\r'),
                            b't' => out.push('\t'),
                            b'b' => out.push('\u{0008}'),
                            b'f' => out.push('\u{000C}'),
                            b'u' => {
                                let hex = self
                                    .bytes
                                    .get(self.pos..self.pos + 4)
                                    .ok_or_else(|| self.error("\\u escape"))?;
                                let hex = core::str::from_utf8(hex)
                                    .map_err(|_| self.error("\\u escape"))?;
                                let code = u32::from_str_radix(hex, 16)
                                    .map_err(|_| self.error("\\u escape"))?;
                                self.pos += 4;
                                // Surrogate pairs: JSON encodes astral chars as
                                // \uD800-\uDBFF followed by \uDC00-\uDFFF.
                                let ch = if (0xD800..0xDC00).contains(&code) {
                                    if !(self.literal("\\u")) {
                                        return Err(self.error("lone high surrogate"));
                                    }
                                    let hex2 = self
                                        .bytes
                                        .get(self.pos..self.pos + 4)
                                        .ok_or_else(|| self.error("\\u escape"))?;
                                    let hex2 = core::str::from_utf8(hex2)
                                        .map_err(|_| self.error("\\u escape"))?;
                                    let low = u32::from_str_radix(hex2, 16)
                                        .map_err(|_| self.error("\\u escape"))?;
                                    self.pos += 4;
                                    if !(0xDC00..0xE000).contains(&low) {
                                        return Err(self.error("invalid low surrogate"));
                                    }
                                    0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00)
                                } else {
                                    code
                                };
                                out.push(
                                    char::from_u32(ch)
                                        .ok_or_else(|| self.error("invalid code point"))?,
                                );
                            }
                            _ => return Err(self.error("unknown escape")),
                        }
                    }
                    _ => {
                        // Re-consume the full UTF-8 character starting at b.
                        self.pos -= 1;
                        let rest = &self.bytes[self.pos..];
                        let s = core::str::from_utf8(rest).map_err(|_| self.error("utf-8"))?;
                        let ch = s.chars().next().ok_or_else(|| self.error("utf-8"))?;
                        out.push(ch);
                        self.pos += ch.len_utf8();
                    }
                }
            }
        }
    }
}
