/// Utility types for the IPC bus.

use std::fmt;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// 4-byte alignment utility for wire format padding.
pub trait Align4 {
    fn align4(self) -> Self;
}

impl Align4 for usize {
    fn align4(mut self) -> Self {
        if (self & 0x3) != 0 {
            self = (self & !0x3) + 4;
        }
        self
    }
}

/// Unique identifier for each endpoint, assigned by the controller on connection.
#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct EndpointID([u8; 16]);

impl EndpointID {
    /// Generate a new random endpoint ID.
    pub fn new() -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&n.to_le_bytes());
        bytes[8..16].copy_from_slice(&n.wrapping_mul(0x9e3779b97f4a7c15).to_le_bytes());
        Self(bytes)
    }
}

impl fmt::Debug for EndpointID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EndpointID({:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x})",
            self.0[0], self.0[1], self.0[2], self.0[3],
            self.0[4], self.0[5], self.0[6], self.0[7],
            self.0[8], self.0[9], self.0[10], self.0[11],
            self.0[12], self.0[13], self.0[14], self.0[15]
        )
    }
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
    fn endpoint_id_unique() {
        let id1 = EndpointID::new();
        let id2 = EndpointID::new();
        assert_ne!(id1, id2);
    }
}
