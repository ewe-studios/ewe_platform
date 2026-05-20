//! Single-threaded (non-multi) pool lifecycle stub.
//!
//! In non-multi builds no OS threads are spawned, so `PoolGuard` is a no-op.

/// Dummy guard for single-threaded builds. Does nothing on drop.
#[derive(Default)]
pub struct PoolGuard;

impl PoolGuard {
    #[must_use]
    pub fn dummy() -> Self {
        Self
    }

    pub fn shutdown(&self) {}
}
