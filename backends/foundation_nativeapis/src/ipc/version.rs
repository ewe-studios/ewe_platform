/// Version compatibility protocol for the IPC bus.
///
/// Every message carries a version header. Endpoints refuse to communicate
/// with incompatible versions, preventing silent data corruption.

use std::fmt::{self, Display, Formatter};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

/// Semver-style version triple.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Version((u8, u8, u8));

impl Version {
    pub fn new() -> Self {
        Self((0, 0, 0))
    }

    pub fn from_parts(major: u8, minor: u8, patch: u8) -> Self {
        Self((major, minor, patch))
    }

    pub fn compatible(&self, rhs: Self) -> bool {
        if self.major() == 0 && rhs.major() == 0 {
            self.minor() == rhs.minor()
        } else {
            self.major() == rhs.major()
        }
    }

    pub fn major(&self) -> u8 {
        self.0 .0
    }

    pub fn minor(&self) -> u8 {
        self.0 .1
    }

    pub fn patch(&self) -> u8 {
        self.0 .2
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0 .0, self.0 .1, self.0 .2)
    }
}

static VERSION: Lazy<Version> = Lazy::new(|| {
    let v_major = env!("CARGO_PKG_VERSION_MAJOR");
    let v_minor = env!("CARGO_PKG_VERSION_MINOR");
    let v_patch = env!("CARGO_PKG_VERSION_PATCH");
    Version((
        v_major.parse().unwrap(),
        v_minor.parse().unwrap(),
        v_patch.parse().unwrap(),
    ))
});

pub fn version() -> Version {
    *VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pre_zero_minor_must_match() {
        let v1 = Version((0, 2, 0));
        let v2 = Version((0, 2, 5));
        let v3 = Version((0, 3, 0));
        assert!(v1.compatible(v2));
        assert!(!v1.compatible(v3));
    }

    #[test]
    fn post_one_major_must_match() {
        let v1 = Version((1, 0, 0));
        let v2 = Version((1, 5, 0));
        let v3 = Version((2, 0, 0));
        assert!(v1.compatible(v2));
        assert!(!v1.compatible(v3));
    }

    #[test]
    fn zero_vs_one() {
        let v1 = Version((0, 5, 0));
        let v2 = Version((1, 0, 0));
        assert!(!v1.compatible(v2));
    }

    #[test]
    fn version_display() {
        let v = Version((1, 2, 3));
        assert_eq!(format!("{}", v), "1.2.3");
    }
}
