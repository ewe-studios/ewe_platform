//! Header and query parsing for the Durable Streams HTTP adapter.

use core::time::Duration;
use foundation_core::durable::{
    BadRequestKind, Producer, ProducerId, ReadFrom, StoreError, StreamOffset, StreamSeq,
};

/// The maximum safe integer for producer fields.
const MAX_SAFE_INTEGER: u64 = (1_u64 << 53) - 1;

/// Parse `Stream-Closed`. Only the exact value `true` counts, case-insensitive.
#[must_use]
pub fn parse_stream_closed(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

/// Parse `Stream-TTL` seconds. The value is a non-negative decimal integer with no leading zeros.
///
/// # Errors
///
/// Returns [`StoreError::BadRequest`] with [`BadRequestKind::Header`] for a malformed value.
pub fn parse_ttl(value: &str) -> Result<Duration, StoreError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(StoreError::BadRequest(BadRequestKind::Header));
    }
    if value.len() > 1 && value.starts_with('0') {
        return Err(StoreError::BadRequest(BadRequestKind::Header));
    }
    let secs = value
        .parse::<u64>()
        .map_err(|_| StoreError::BadRequest(BadRequestKind::Header))?;
    Ok(Duration::from_secs(secs))
}

/// Parse one producer header set. All three come together or none.
///
/// # Errors
///
/// Returns [`StoreError::BadRequest`] with [`BadRequestKind::Producer`] for a partial or invalid
/// set.
pub fn parse_producer(
    id: Option<&str>,
    epoch: Option<&str>,
    seq: Option<&str>,
) -> Result<Option<Producer>, StoreError> {
    match (id, epoch, seq) {
        (None, None, None) => Ok(None),
        (Some(id), Some(epoch), Some(seq)) => Ok(Some(Producer {
            id: ProducerId::parse(id)?,
            epoch: parse_safe_integer(epoch)?,
            seq: parse_safe_integer(seq)?,
        })),
        _ => Err(StoreError::BadRequest(BadRequestKind::Producer)),
    }
}

fn parse_safe_integer(value: &str) -> Result<u64, StoreError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(StoreError::BadRequest(BadRequestKind::Producer));
    }
    let value = value
        .parse::<u64>()
        .map_err(|_| StoreError::BadRequest(BadRequestKind::Producer))?;
    if value > MAX_SAFE_INTEGER {
        return Err(StoreError::BadRequest(BadRequestKind::Producer));
    }
    Ok(value)
}

/// Parse `Stream-Fork-Sub-Offset`. The value is a decimal integer with no
/// sign and no leading zero.
///
/// # Errors
/// Returns [`StoreError::BadRequest`] with [`BadRequestKind::Fork`] for a
/// malformed value.
pub fn parse_sub_offset(value: &str) -> Result<u32, StoreError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(StoreError::BadRequest(BadRequestKind::Fork));
    }
    if value.len() > 1 && value.starts_with('0') {
        return Err(StoreError::BadRequest(BadRequestKind::Fork));
    }
    value
        .parse::<u32>()
        .map_err(|_| StoreError::BadRequest(BadRequestKind::Fork))
}

/// Parse `Stream-Expires-At` as an RFC 3339 timestamp into UNIX milliseconds.
///
/// # Errors
/// Returns [`StoreError::BadRequest`] with [`BadRequestKind::Header`] for a
/// malformed timestamp.
pub fn parse_expires_at(value: &str) -> Result<i64, StoreError> {
    let at = time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
        .map_err(|_| StoreError::BadRequest(BadRequestKind::Header))?;
    i64::try_from(at.unix_timestamp_nanos() / 1_000_000)
        .map_err(|_| StoreError::BadRequest(BadRequestKind::Header))
}

/// Parse `Stream-Seq`. The value is opaque.
#[must_use]
pub fn parse_stream_seq(value: &str) -> StreamSeq {
    StreamSeq::from_bytes(value.as_bytes().to_vec())
}

/// Parse a read offset. `-1` means the start. `now` means the tail.
///
/// Any other value is the wire form of an internal offset, which
/// [`encode_offset`] produces.
///
/// # Errors
///
/// Returns [`StoreError::BadRequest`] with [`BadRequestKind::Offset`] for an empty or malformed
/// value.
pub fn parse_offset(value: &str) -> Result<ReadFrom, StoreError> {
    match value {
        "-1" => Ok(ReadFrom::Start),
        "now" => Ok(ReadFrom::Tail),
        "" => Err(StoreError::BadRequest(BadRequestKind::Offset)),
        other => Ok(ReadFrom::Offset(decode_offset(other)?)),
    }
}

/// Encode an internal offset as wire text.
///
/// The wire form is lowercase hexadecimal, two characters per byte. The
/// encoding is fixed width, so wire text sorts in the same order as the raw
/// bytes. A client passes the text back unchanged.
#[must_use]
pub fn encode_offset(offset: &StreamOffset) -> String {
    let mut text = String::with_capacity(offset.as_bytes().len() * 2);
    for byte in offset.as_bytes() {
        text.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        text.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    text
}

/// Decode wire text into the internal offset bytes.
///
/// # Errors
///
/// Returns [`StoreError::BadRequest`] with [`BadRequestKind::Offset`] when the
/// text is not an even-length, lowercase hexadecimal string.
pub fn decode_offset(value: &str) -> Result<StreamOffset, StoreError> {
    if let Some((sequence, position)) = value.split_once('_') {
        // The reference server writes a composite `<sequence>_<byte>` offset.
        // This store orders by byte position, so the byte part is the offset.
        if sequence.is_empty() || position.is_empty() {
            return Err(StoreError::BadRequest(BadRequestKind::Offset));
        }
        if sequence.bytes().all(|byte| byte == b'0') && position.bytes().all(|byte| byte == b'0') {
            return Ok(StreamOffset::from_bytes(vec![0; 8]));
        }
        let byte = position
            .parse::<u64>()
            .map_err(|_| StoreError::BadRequest(BadRequestKind::Offset))?;
        return Ok(StreamOffset::from_bytes(byte.to_be_bytes().to_vec()));
    }
    if value.is_empty() || value.len() % 2 != 0 {
        return Err(StoreError::BadRequest(BadRequestKind::Offset));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let mut chunks = value.as_bytes().chunks_exact(2);
    for chunk in &mut chunks {
        let high = hex_digit(chunk[0])?;
        let low = hex_digit(chunk[1])?;
        bytes.push((high << 4) | low);
    }
    if !chunks.remainder().is_empty() {
        return Err(StoreError::BadRequest(BadRequestKind::Offset));
    }
    Ok(StreamOffset::from_bytes(bytes))
}

fn hex_digit(byte: u8) -> Result<u8, StoreError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(StoreError::BadRequest(BadRequestKind::Offset)),
    }
}
