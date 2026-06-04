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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align4_no_padding() {
        assert_eq!(4usize.align4(), 4);
        assert_eq!(8usize.align4(), 8);
        assert_eq!(0usize.align4(), 0);
    }

    #[test]
    fn align4_padded() {
        assert_eq!(1usize.align4(), 4);
        assert_eq!(2usize.align4(), 4);
        assert_eq!(3usize.align4(), 4);
        assert_eq!(5usize.align4(), 8);
        assert_eq!(7usize.align4(), 8);
    }

    #[test]
    fn align4_u32() {
        assert_eq!(1u32.align4(), 4);
        assert_eq!(4u32.align4(), 4);
    }

    #[test]
    fn endpoint_id_unique() {
        let id1 = EndpointID::new();
        let id2 = EndpointID::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn range_to_offset_size_unbounded() {
        let (offset, size) = range_to_offset_size(..);
        assert_eq!(offset, 0);
        assert_eq!(size, None);
    }

    #[test]
    fn range_to_offset_size_exclusive() {
        let (offset, size) = range_to_offset_size(2..8);
        assert_eq!(offset, 2);
        assert_eq!(size, Some(6));
    }

    #[test]
    fn range_to_offset_size_inclusive() {
        let (offset, size) = range_to_offset_size(2..=7);
        assert_eq!(offset, 2);
        assert_eq!(size, Some(6));
    }
}
