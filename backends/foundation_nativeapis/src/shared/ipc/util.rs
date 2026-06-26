/// Utility types for the IPC bus.

use std::ops::{Bound, RangeBounds};

use serde::{Deserialize, Serialize};
use foundation_core::type_uuid::Bytes;

/// 4-byte alignment utility for wire format padding.
pub trait Align4 {
    fn align4(self) -> Self;
}

impl Align4 for usize {
    #[inline]
    fn align4(mut self) -> Self {
        if (self & 0x3) != 0 {
            self = (self & !0x3) + 4;
        }
        self
    }
}

impl Align4 for u32 {
    #[inline]
    fn align4(mut self) -> Self {
        if (self & 0x3) != 0 {
            self = (self & !0x3) + 4;
        }
        self
    }
}

/// Unique identifier for each endpoint, assigned by the controller on connection.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct EndpointID(Bytes);

impl EndpointID {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().into_bytes())
    }
}

/// Convert a range to (offset, optional_size) tuple.
pub fn range_to_offset_size<S: RangeBounds<usize>>(bounds: S) -> (usize, Option<usize>) {
    let offset = match bounds.start_bound() {
        Bound::Included(&bound) => bound,
        Bound::Excluded(&bound) => bound + 1,
        Bound::Unbounded => 0,
    };
    let size = match bounds.end_bound() {
        Bound::Included(&bound) => Some(bound + 1 - offset),
        Bound::Excluded(&bound) => Some(bound - offset),
        Bound::Unbounded => None,
    };

    (offset, size)
}

