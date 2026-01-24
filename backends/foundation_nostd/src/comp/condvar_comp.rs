//! CondVar compatibility layer for std and no_std.
//!
//! This module provides properly paired Mutex and CondVar types that work together
//! in both std and no_std environments.
//!
//! # Why This Module?
//!
//! The standard `comp::Mutex` uses SpinMutex in no_std mode, but CondVar requires
//! CondVarMutex for guard type compatibility. This module ensures the types are
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

/// Mutex type for use with CondVar.
///
/// - With `std` feature: Uses `std::sync::Mutex`
/// - Without `std` feature: Uses `foundation_nostd::primitives::CondVarMutex`
#[cfg(feature = "std")]
pub use std::sync::Mutex;

/// Mutex type for use with CondVar.
///
/// - With `std` feature: Uses `std::sync::Mutex`
/// - Without `std` feature: Uses `foundation_nostd::primitives::CondVarMutex`
#[cfg(not(feature = "std"))]
pub use crate::primitives::condvar::CondVarMutex as Mutex;

/// Mutex guard type for use with CondVar.
///
/// - With `std` feature: Uses `std::sync::MutexGuard`
/// - Without `std` feature: Uses `foundation_nostd::primitives::CondVarMutexGuard`
#[cfg(feature = "std")]
pub use std::sync::MutexGuard;

/// Mutex guard type for use with CondVar.
///
/// - With `std` feature: Uses `std::sync::MutexGuard`
/// - Without `std` feature: Uses `foundation_nostd::primitives::CondVarMutexGuard`
#[cfg(not(feature = "std"))]
pub use crate::primitives::condvar::CondVarMutexGuard as MutexGuard;

// ============================================================================
// CondVar
// ============================================================================

/// Platform-appropriate CondVar type.
///
/// - With `std` feature: Uses `std::sync::Condvar`
/// - Without `std` feature: Uses `foundation_nostd::primitives::CondVar`
#[cfg(feature = "std")]
pub use std::sync::Condvar as CondVar;

/// Platform-appropriate CondVar type.
///
/// - With `std` feature: Uses `std::sync::Condvar`
/// - Without `std` feature: Uses `foundation_nostd::primitives::CondVar`
#[cfg(not(feature = "std"))]
pub use crate::primitives::CondVar;

/// Platform-appropriate WaitTimeoutResult type.
///
/// - With `std` feature: Uses `std::sync::WaitTimeoutResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::WaitTimeoutResult`
#[cfg(feature = "std")]
pub use std::sync::WaitTimeoutResult;

/// Platform-appropriate WaitTimeoutResult type.
///
/// - With `std` feature: Uses `std::sync::WaitTimeoutResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::WaitTimeoutResult`
#[cfg(not(feature = "std"))]
pub use crate::primitives::WaitTimeoutResult;
