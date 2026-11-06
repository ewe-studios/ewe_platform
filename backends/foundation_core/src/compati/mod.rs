//! Sub module to provide compatibility layer for specific types to use in specific platforms.

#[cfg(target_family = "wasm")]
pub use foundation_nostd::primitives::Mutex;

#[cfg(not(target_family = "wasm"))]
pub use std::sync::Mutex;

#[cfg(target_family = "wasm")]
pub use foundation_nostd::primitives::RwLock;

#[cfg(not(target_family = "wasm"))]
pub use std::sync::RwLock;

// Properly-paired condvar + mutex for park/wake. The plain `Mutex` above is a
// Noop/Spin mutex on wasm and is NOT guard-compatible with a condvar — code that
// needs `Condvar::wait` must use `CondVarMutex`, not `Mutex`. Both are cfg-gated
// inside foundation_nostd (`std::sync` on native, spin-wait on single-threaded
// wasm), so this re-export is platform-correct without further cfg here.
pub use foundation_nostd::comp::condvar_comp::{CondVar as Condvar, CondVarMutex};

/// Recover the guarded value from a poisoned lock.
///
/// WHY: the poison type differs by target. Native uses
/// [`std::sync::PoisonError`]; wasm uses
/// [`foundation_nostd::primitives::PoisonError`]. The closure
/// `|e| e.into_inner()` works on both, but clippy's `redundant_closure` rejects
/// it on native, and the path it suggests
/// (`std::sync::PoisonError::into_inner`) does not compile on wasm. One name
/// for the operation on both targets avoids both problems.
#[inline]
pub(crate) fn recover_poison<T>(error: impl RecoverPoison<T>) -> T {
    error.recover_poison()
}

/// The target-specific poison-error recovery behind [`recover_poison`].
pub(crate) trait RecoverPoison<T> {
    /// Return the guarded value, ignoring the poison flag.
    fn recover_poison(self) -> T;
}

// Both poison types exist on every target; which one a lock returns depends on
// the lock implementation, not only on the target. Implement both so the helper
// covers `std` locks and `foundation_nostd` locks everywhere.
impl<T> RecoverPoison<T> for std::sync::PoisonError<T> {
    #[inline]
    fn recover_poison(self) -> T {
        self.into_inner()
    }
}

impl<T> RecoverPoison<T> for foundation_nostd::primitives::PoisonError<T> {
    #[inline]
    fn recover_poison(self) -> T {
        self.into_inner()
    }
}
