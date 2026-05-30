//! `CondVar` compatibility layer for std and `no_std`.
//!
//! This module provides properly paired Mutex and `CondVar` types that work together
//! in both std and `no_std` environments.
//!
//! # Why This Module?
//!
//! The standard `comp::Mutex` uses `SpinMutex` in `no_std` mode, but `CondVar` requires
//! `CondVarMutex` for guard type compatibility. This module ensures the types are
//! properly paired.
//!
//! # Examples
//!
//! ```no_run
//! use foundation_nostd::comp::condvar_comp::{Mutex, CondVar};
//!
//! let mutex = Mutex::new(false);
//! let condvar = CondVar::new();
//!
//! let mut guard = mutex.lock().unwrap();
//! while !*guard {
//!     guard = condvar.wait(guard).unwrap();
//! }
//! ```

// ============================================================================
// CondVar-Compatible Mutex
// ============================================================================

/// `CondVarMutex` type for use with `CondVar`.
///
/// - With `js-wasmbindgen` or no `std`: Uses `foundation_nostd::primitives::CondVarMutex`
/// - With `std` only: Uses `std::sync::Mutex`
#[cfg(all(feature = "std", not(feature = "js-wasmbindgen")))]
pub use std::sync::Mutex as CondVarMutex;

/// `CondVarMutex` type for use with `CondVar`.
///
/// - With `js-wasmbindgen` or no `std`: Uses `foundation_nostd::primitives::CondVarMutex`
/// - With `std` only: Uses `std::sync::Mutex`
#[cfg(any(not(feature = "std"), feature = "js-wasmbindgen"))]
pub use crate::primitives::condvar::CondVarMutex;

/// Mutex type for use with `CondVar`.
///
/// - With `js-wasmbindgen` or no `std`: Uses `foundation_nostd::primitives::CondVarMutex`
/// - With `std` only: Uses `std::sync::Mutex`
#[cfg(all(feature = "std", not(feature = "js-wasmbindgen")))]
pub use std::sync::Mutex;

/// Mutex type for use with `CondVar`.
///
/// - With `js-wasmbindgen` or no `std`: Uses `foundation_nostd::primitives::CondVarMutex`
/// - With `std` only: Uses `foundation_nostd::primitives::CondVarMutex`
#[cfg(any(not(feature = "std"), feature = "js-wasmbindgen"))]
pub use crate::primitives::condvar::CondVarMutex as Mutex;

/// Mutex guard type for use with `CondVar`.
///
/// - With `js-wasmbindgen` or no `std`: Uses `foundation_nostd::primitives::CondVarMutexGuard`
/// - With `std` only: Uses `std::sync::MutexGuard`
#[cfg(all(feature = "std", not(feature = "js-wasmbindgen")))]
pub use std::sync::MutexGuard;

/// Mutex guard type for use with `CondVar`.
///
/// - With `js-wasmbindgen` or no `std`: Uses `foundation_nostd::primitives::CondVarMutexGuard`
/// - With `std` only: Uses `std::sync::MutexGuard`
#[cfg(any(not(feature = "std"), feature = "js-wasmbindgen"))]
pub use crate::primitives::condvar::CondVarMutexGuard as MutexGuard;

// ============================================================================
// CondVar
// ============================================================================

/// Platform-appropriate `CondVar` type.
///
/// - With `js-wasmbindgen` or no `std`: Uses `foundation_nostd::primitives::CondVar`
/// - With `std` only: Uses `std::sync::Condvar`
#[cfg(all(feature = "std", not(feature = "js-wasmbindgen")))]
pub use std::sync::Condvar as CondVar;

/// Platform-appropriate `CondVar` type.
///
/// - With `js-wasmbindgen` or no `std`: Uses `foundation_nostd::primitives::CondVar`
/// - With `std` only: Uses `std::sync::Condvar`
#[cfg(any(not(feature = "std"), feature = "js-wasmbindgen"))]
pub use crate::primitives::CondVar;

/// Platform-appropriate `WaitTimeoutResult` type.
///
/// - With `js-wasmbindgen` or no `std`: Uses `foundation_nostd::primitives::WaitTimeoutResult`
/// - With `std` only: Uses `std::sync::WaitTimeoutResult`
#[cfg(all(feature = "std", not(feature = "js-wasmbindgen")))]
pub use std::sync::WaitTimeoutResult;

/// Platform-appropriate `WaitTimeoutResult` type.
///
/// - With `js-wasmbindgen` or no `std`: Uses `foundation_nostd::primitives::WaitTimeoutResult`
/// - With `std` only: Uses `std::sync::WaitTimeoutResult`
#[cfg(any(not(feature = "std"), feature = "js-wasmbindgen"))]
pub use crate::primitives::WaitTimeoutResult;
