//! Offset helpers shared by the subscription core.
//!
//! An acknowledged cursor is wire text. `-1` means before the first offset, so the next read
//! starts at the beginning. Every other value is a core offset in the fixed-width hexadecimal
//! form that [`foundation_core::durable::wire`] produces.

use foundation_core::durable::{wire, StreamOffset, VfsOffset};
use std::cmp::Ordering;

/// The offset before the first byte.
pub const BEFORE_FIRST: &str = "-1";

/// The wire text of a zero offset.
#[must_use]
pub fn zero_offset() -> String {
    wire::offset_text(&StreamOffset::from(VfsOffset::new(0)))
}

/// The wire text of the offset at `len`.
#[must_use]
pub fn offset_from_len(len: u64) -> String {
    wire::offset_text(&StreamOffset::from(VfsOffset::new(len)))
}

/// Compare two wire offsets.
///
/// `-1` sorts before every real offset. A value that does not parse compares as text, so a
/// malformed cursor never advances.
#[must_use]
pub fn compare_offsets(left: &str, right: &str) -> Ordering {
    match (left, right) {
        (BEFORE_FIRST, BEFORE_FIRST) => Ordering::Equal,
        (BEFORE_FIRST, _) => Ordering::Less,
        (_, BEFORE_FIRST) => Ordering::Greater,
        _ => match (wire::parse_offset(left), wire::parse_offset(right)) {
            (Some(left), Some(right)) => left.cmp(&right),
            _ => left.cmp(right),
        },
    }
}

/// True when `offset` is at or before the zero offset and is not the before-first marker.
#[must_use]
pub fn is_zero(offset: &str) -> bool {
    compare_offsets(offset, &zero_offset()) == Ordering::Equal
}
