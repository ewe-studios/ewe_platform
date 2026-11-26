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

/// Parse `Stream-Seq`. The value is opaque.
#[must_use]
pub fn parse_stream_seq(value: &str) -> StreamSeq {
    StreamSeq::from_bytes(value.as_bytes().to_vec())
}

/// Parse a read offset. `-1` means the start. `now` means the tail.
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
        other => {
            if other
                .bytes()
                .any(|byte| matches!(byte, b',' | b'&' | b'=' | b'?' | b'/'))
            {
                return Err(StoreError::BadRequest(BadRequestKind::Offset));
            }
            Ok(ReadFrom::Offset(StreamOffset::from_bytes(
                other.as_bytes().to_vec(),
            )))
        }
    }
}
