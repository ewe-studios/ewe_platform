//! The wire text of a stream offset.
//!
//! Both the server adapter and the client use this codec, so an offset the
//! server sends is the offset the client sends back.

use super::StreamOffset;

/// Encode an offset as wire text.
///
/// The form is lowercase hexadecimal, two characters per byte. The width is
/// fixed, so the text sorts in the same order as the bytes.
#[must_use]
pub fn offset_text(offset: &StreamOffset) -> String {
    let mut text = String::with_capacity(offset.as_bytes().len() * 2);
    for byte in offset.as_bytes() {
        text.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        text.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    text
}

/// Decode wire text into an offset.
///
/// The decoder accepts this module's hexadecimal form and the reference
/// server's `<sequence>_<byte>` form. It returns `None` for malformed text.
#[must_use]
pub fn parse_offset(value: &str) -> Option<StreamOffset> {
    if let Some((sequence, position)) = value.split_once('_') {
        if sequence.is_empty() || position.is_empty() {
            return None;
        }
        if sequence.bytes().all(|byte| byte == b'0') && position.bytes().all(|byte| byte == b'0') {
            return Some(StreamOffset::from_bytes(vec![0; 8]));
        }
        let byte = position.parse::<u64>().ok()?;
        return Some(StreamOffset::from_bytes(byte.to_be_bytes().to_vec()));
    }
    if value.is_empty() || value.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let mut chunks = value.as_bytes().chunks_exact(2);
    for chunk in &mut chunks {
        bytes.push((hex_digit(chunk[0])? << 4) | hex_digit(chunk[1])?);
    }
    if !chunks.remainder().is_empty() {
        return None;
    }
    Some(StreamOffset::from_bytes(bytes))
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
