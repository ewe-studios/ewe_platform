//! Web-specific extensions to [`foundation_compact`].

use std::time::SystemTime as StdSystemTime;

use crate::SystemTime;

/// Web-specific extension to [`foundation_compact::SystemTime`].
pub trait SystemTimeExt {
    /// Convert [`foundation_compact::SystemTime`] to [`std::time::SystemTime`].
    ///
    /// # Note
    ///
    /// This might give a misleading impression of compatibility!
    /// Considering this functionality will probably be used to interact with
    /// incompatible APIs of other dependencies, care should be taken that the
    /// dependency in question doesn't call [`std::time::SystemTime::now()`]
    /// internally, which would panic on wasm32-unknown-unknown.
    fn to_std(self) -> std::time::SystemTime;

    /// Convert [`std::time::SystemTime`] to [`foundation_compact::SystemTime`].
    ///
    /// # Note
    ///
    /// This might give a misleading impression of compatibility!
    /// Considering this functionality will probably be used to interact with
    /// incompatible APIs of other dependencies, care should be taken that the
    /// dependency in question doesn't call [`std::time::SystemTime::now()`]
    /// internally, which would panic on wasm32-unknown-unknown.
    fn from_std(time: std::time::SystemTime) -> SystemTime;
}

impl SystemTimeExt for SystemTime {
    fn to_std(self) -> std::time::SystemTime {
        StdSystemTime::UNIX_EPOCH + self.0
    }

    fn from_std(time: std::time::SystemTime) -> SystemTime {
        Self::UNIX_EPOCH
            + time
                .duration_since(StdSystemTime::UNIX_EPOCH)
                .expect("found `SystemTime` earlier than unix epoch")
    }
}
