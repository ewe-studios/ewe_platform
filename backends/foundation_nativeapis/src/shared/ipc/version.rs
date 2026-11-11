/// Version compatibility protocol for the IPC bus.
///
/// Every message carries a version header. Endpoints refuse to communicate
/// with incompatible versions, preventing silent data corruption.
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Semver-style version triple.
#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct Version((u8, u8, u8));

impl Version {
    /// Create the zero version, `0.0.0`.
    #[must_use]
    pub fn new() -> Self {
        Self((0, 0, 0))
    }

    /// Create a version from its parts.
    #[must_use]
    pub fn from_parts(major: u8, minor: u8, patch: u8) -> Self {
        Self((major, minor, patch))
    }

    /// Check if `rhs` can talk to this version.
    #[must_use]
    pub fn compatible(&self, rhs: Self) -> bool {
        if self.major() == 0 && rhs.major() == 0 {
            self.minor() == rhs.minor()
        } else {
            self.major() == rhs.major()
        }
    }

    /// The major part.
    #[must_use]
    pub fn major(&self) -> u8 {
        self.0 .0
    }

    /// The minor part.
    #[must_use]
    pub fn minor(&self) -> u8 {
        self.0 .1
    }

    /// The patch part.
    #[must_use]
    pub fn patch(&self) -> u8 {
        self.0 .2
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0 .0, self.0 .1, self.0 .2)
    }
}

static VERSION: LazyLock<Version> = LazyLock::new(|| {
    let v_major = env!("CARGO_PKG_VERSION_MAJOR");
    let v_minor = env!("CARGO_PKG_VERSION_MINOR");
    let v_patch = env!("CARGO_PKG_VERSION_PATCH");
    Version((
        v_major.parse().unwrap(),
        v_minor.parse().unwrap(),
        v_patch.parse().unwrap(),
    ))
});

static VERSION_PRE: LazyLock<&'static str> = LazyLock::new(|| env!("CARGO_PKG_VERSION_PRE"));

/// The version of this crate.
#[must_use]
pub fn version() -> Version {
    *VERSION
}

/// The pre-release part of the version of this crate.
#[must_use]
pub fn version_pre() -> String {
    VERSION_PRE.to_owned()
}
