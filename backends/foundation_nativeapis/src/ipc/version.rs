/// Version compatibility protocol for the IPC bus.
///
/// Every message carries a version header. Endpoints refuse to communicate
/// with incompatible versions, preventing silent data corruption.
///
/// Compatibility rules:
/// - If both major versions are 0 → minor versions must match (pre-1.0 breaking changes)
/// - If either major version is >= 1 → major versions must match (semver)

use serde::{Deserialize, Serialize};

/// Version read from crate metadata at compile time.
pub fn version() -> Version {
    Version::from_str_parts(
        env!("CARGO_PKG_VERSION_MAJOR")
            .parse()
            .expect("invalid major version"),
        env!("CARGO_PKG_VERSION_MINOR")
            .parse()
            .expect("invalid minor version"),
        env!("CARGO_PKG_VERSION_PATCH")
            .parse()
            .expect("invalid patch version"),
    )
}

/// A semver-style version triple.
///
/// Wire format: `[magic: u8 = 0xFF][major: u8][minor: u8][patch: u8]` — packed as `u32`.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Version((u8, u8, u8));

impl Version {
    /// Magic byte prefix for wire format.
    pub const MAGIC: u8 = 0xFF;

    /// Create a version from parts.
    pub fn from_str_parts(major: u8, minor: u8, patch: u8) -> Self {
        Self((major, minor, patch))
    }

    /// Major version.
    pub fn major(&self) -> u8 {
        self.0 .0
    }

    /// Minor version.
    pub fn minor(&self) -> u8 {
        self.0 .1
    }

    /// Patch version.
    pub fn patch(&self) -> u8 {
        self.0 .2
    }

    /// Check if this version is compatible with another.
    ///
    /// - If both major == 0 → minor must match
    /// - Otherwise → major must match
    pub fn compatible(&self, rhs: Self) -> bool {
        if self.major() == 0 && rhs.major() == 0 {
            self.minor() == rhs.minor()
        } else {
            self.major() == rhs.major()
        }
    }

    /// Encode as u32: `[0xFF][major][minor][patch]`.
    pub fn to_u32(&self) -> u32 {
        ((Self::MAGIC as u32) << 24)
            | ((self.major() as u32) << 16)
            | ((self.minor() as u32) << 8)
            | (self.patch() as u32)
    }

    /// Decode from u32: `[0xFF][major][minor][patch]`.
    pub fn from_u32(val: u32) -> Option<Self> {
        let magic = ((val >> 24) & 0xFF) as u8;
        if magic != Self::MAGIC {
            return None;
        }
        Some(Self((
            ((val >> 16) & 0xFF) as u8,
            ((val >> 8) & 0xFF) as u8,
            (val & 0xFF) as u8,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pre_zero_minor_must_match() {
        let v1 = Version::from_str_parts(0, 2, 0);
        let v2 = Version::from_str_parts(0, 2, 5);
        let v3 = Version::from_str_parts(0, 3, 0);
        assert!(v1.compatible(v2));
        assert!(!v1.compatible(v3));
    }

    #[test]
    fn post_one_major_must_match() {
        let v1 = Version::from_str_parts(1, 0, 0);
        let v2 = Version::from_str_parts(1, 5, 0);
        let v3 = Version::from_str_parts(2, 0, 0);
        assert!(v1.compatible(v2));
        assert!(!v1.compatible(v3));
    }

    #[test]
    fn zero_vs_one() {
        let v1 = Version::from_str_parts(0, 5, 0);
        let v2 = Version::from_str_parts(1, 0, 0);
        // v1.major == 0, v2.major == 1 → major must match → 0 != 1 → incompatible
        assert!(!v1.compatible(v2));
    }

    #[test]
    fn encode_decode_roundtrip() {
        let v = Version::from_str_parts(1, 2, 3);
        let encoded = v.to_u32();
        let decoded = Version::from_u32(encoded).unwrap();
        assert_eq!(v, decoded);
        assert_eq!(decoded.major(), 1);
        assert_eq!(decoded.minor(), 2);
        assert_eq!(decoded.patch(), 3);
    }

    #[test]
    fn invalid_magic() {
        assert!(Version::from_u32(0x00_01_02_03).is_none());
    }
}
