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
