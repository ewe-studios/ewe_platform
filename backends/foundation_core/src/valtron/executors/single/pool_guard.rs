//! Single-threaded (non-multi) pool lifecycle stub.
//!
//! In non-multi builds no OS threads are spawned, so `PoolGuard` is a no-op.

/// Dummy guard for single-threaded builds. Does nothing on drop.
#[derive(Default, Clone)]
pub struct PoolGuard;

impl PoolGuard {
    #[must_use]
    pub fn dummy() -> Self {
        Self
    }

    pub fn shutdown(&self) {}

    /// No pool threads exist, so there is nothing to wait for: always `true`.
    /// Mirrors the multi-threaded `PoolGuard::shutdown_within`.
    #[must_use]
    pub fn shutdown_within(&self, _timeout: core::time::Duration) -> bool {
        true
    }
}
