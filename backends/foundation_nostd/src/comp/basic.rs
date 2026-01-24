//! Basic synchronization primitives for std and no_std.
//!
//! This module provides type aliases that automatically select between `std` and `no_std`
//! implementations based on the `std` feature flag.
//!
//! # Types Provided
//!
//! - `Mutex`, `MutexGuard` - Mutual exclusion primitives
//! - `RwLock`, `RwLockReadGuard`, `RwLockWriteGuard` - Reader-writer locks
//! - `Barrier`, `BarrierWaitResult` - Synchronization barriers
//! - `Once`, `OnceState` - One-time initialization
//! - `OnceLock` - One-time initialized cells
//! - Error types: `PoisonError`, `TryLockError`, `LockResult`, `TryLockResult`
//!
//! # Feature Flags
//!
//! - `std`: When enabled, uses `std::sync` types for optimal performance
//! - When disabled (default): Uses `foundation_nostd` spin-lock based implementations

// ============================================================================
// Mutex Types
// ============================================================================

/// Platform-appropriate `Mutex` type.
///
/// - With `std` feature: Uses `std::sync::Mutex`
/// - Without `std` feature: Uses `foundation_nostd::primitives::Mutex` (platform-appropriate spin mutex)
#[cfg(feature = "std")]
pub use std::sync::Mutex;

/// Platform-appropriate `Mutex` type.
///
/// - With `std` feature: Uses `std::sync::Mutex`
/// - Without `std` feature: Uses `foundation_nostd::primitives::Mutex` (platform-appropriate spin mutex)
#[cfg(not(feature = "std"))]
pub use crate::primitives::Mutex;

/// Platform-appropriate `MutexGuard` type.
///
/// - With `std` feature: Uses `std::sync::MutexGuard`
/// - Without `std` feature: Uses the guard type from `foundation_nostd::primitives`
#[cfg(feature = "std")]
pub use std::sync::MutexGuard;

// ============================================================================
// RwLock Types
// ============================================================================

/// Platform-appropriate `RwLock` type.
///
/// - With `std` feature: Uses `std::sync::RwLock`
/// - Without `std` feature: Uses `foundation_nostd::primitives::RwLock` (platform-appropriate spin rwlock)
#[cfg(feature = "std")]
pub use std::sync::RwLock;

/// Platform-appropriate `RwLock` type.
///
/// - With `std` feature: Uses `std::sync::RwLock`
/// - Without `std` feature: Uses `foundation_nostd::primitives::RwLock` (platform-appropriate spin rwlock)
#[cfg(not(feature = "std"))]
pub use crate::primitives::RwLock;

/// Platform-appropriate `RwLockReadGuard` type.
///
/// - With `std` feature: Uses `std::sync::RwLockReadGuard`
/// - Without `std` feature: Uses the read guard type from `foundation_nostd::primitives`
#[cfg(feature = "std")]
pub use std::sync::RwLockReadGuard;

/// Platform-appropriate `RwLockWriteGuard` type.
///
/// - With `std` feature: Uses `std::sync::RwLockWriteGuard`
/// - Without `std` feature: Uses the write guard type from `foundation_nostd::primitives`
#[cfg(feature = "std")]
pub use std::sync::RwLockWriteGuard;

// ============================================================================
// Barrier Types
// ============================================================================

/// Platform-appropriate `Barrier` type.
///
/// - With `std` feature: Uses `std::sync::Barrier`
/// - Without `std` feature: Uses `foundation_nostd::primitives::SpinBarrier`
#[cfg(feature = "std")]
pub use std::sync::Barrier;

/// Platform-appropriate `Barrier` type.
///
/// - With `std` feature: Uses `std::sync::Barrier`
/// - Without `std` feature: Uses `foundation_nostd::primitives::SpinBarrier`
#[cfg(not(feature = "std"))]
pub use crate::primitives::SpinBarrier as Barrier;

/// Platform-appropriate `BarrierWaitResult` type.
///
/// - With `std` feature: Uses `std::sync::BarrierWaitResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::BarrierWaitResult`
#[cfg(feature = "std")]
pub use std::sync::BarrierWaitResult;

/// Platform-appropriate `BarrierWaitResult` type.
///
/// - With `std` feature: Uses `std::sync::BarrierWaitResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::BarrierWaitResult`
#[cfg(not(feature = "std"))]
pub use crate::primitives::BarrierWaitResult;

// ============================================================================
// Once Types
// ============================================================================

/// Platform-appropriate `Once` type for one-time initialization.
///
/// - With `std` feature: Uses `std::sync::Once`
/// - Without `std` feature: Uses `foundation_nostd::primitives::Once`
#[cfg(feature = "std")]
pub use std::sync::Once;

/// Platform-appropriate `Once` type for one-time initialization.
///
/// - With `std` feature: Uses `std::sync::Once`
/// - Without `std` feature: Uses `foundation_nostd::primitives::Once`
#[cfg(not(feature = "std"))]
pub use crate::primitives::Once;

/// Platform-appropriate `OnceState` type.
///
/// - With `std` feature: Uses `std::sync::OnceState`
/// - Without `std` feature: Uses `foundation_nostd::primitives::OnceState`
#[cfg(feature = "std")]
pub use std::sync::OnceState;

/// Platform-appropriate `OnceState` type.
///
/// - With `std` feature: Uses `std::sync::OnceState`
/// - Without `std` feature: Uses `foundation_nostd::primitives::OnceState`
#[cfg(not(feature = "std"))]
pub use crate::primitives::OnceState;

// ============================================================================
// OnceLock Types
// ============================================================================

/// Platform-appropriate `OnceLock` type for one-time initialized cells.
///
/// - With `std` feature: Uses `std::sync::OnceLock`
/// - Without `std` feature: Uses `foundation_nostd::primitives::OnceLock`
#[cfg(feature = "std")]
pub use std::sync::OnceLock;

/// Platform-appropriate `OnceLock` type for one-time initialized cells.
///
/// - With `std` feature: Uses `std::sync::OnceLock`
/// - Without `std` feature: Uses `foundation_nostd::primitives::OnceLock`
#[cfg(not(feature = "std"))]
pub use crate::primitives::OnceLock;

// ============================================================================
// Poison Types
// ============================================================================

/// Platform-appropriate `PoisonError` type.
///
/// - With `std` feature: Uses `std::sync::PoisonError`
/// - Without `std` feature: Uses `foundation_nostd::primitives::PoisonError`
#[cfg(feature = "std")]
pub use std::sync::PoisonError;

/// Platform-appropriate `PoisonError` type.
///
/// - With `std` feature: Uses `std::sync::PoisonError`
/// - Without `std` feature: Uses `foundation_nostd::primitives::PoisonError`
#[cfg(not(feature = "std"))]
pub use crate::primitives::PoisonError;

/// Platform-appropriate `TryLockError` type.
///
/// - With `std` feature: Uses `std::sync::TryLockError`
/// - Without `std` feature: Uses `foundation_nostd::primitives::TryLockError`
#[cfg(feature = "std")]
pub use std::sync::TryLockError;

/// Platform-appropriate `TryLockError` type.
///
/// - With `std` feature: Uses `std::sync::TryLockError`
/// - Without `std` feature: Uses `foundation_nostd::primitives::TryLockError`
#[cfg(not(feature = "std"))]
pub use crate::primitives::TryLockError;

/// Type alias for `Result` with `PoisonError`.
///
/// - With `std` feature: Uses `std::sync::LockResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::LockResult`
#[cfg(feature = "std")]
pub use std::sync::LockResult;

/// Type alias for `Result` with `PoisonError`.
///
/// - With `std` feature: Uses `std::sync::LockResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::LockResult`
#[cfg(not(feature = "std"))]
pub use crate::primitives::LockResult;

/// Type alias for `Result` with `TryLockError`.
///
/// - With `std` feature: Uses `std::sync::TryLockResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::TryLockResult`
#[cfg(feature = "std")]
pub use std::sync::TryLockResult;

/// Type alias for `Result` with `TryLockError`.
///
/// - With `std` feature: Uses `std::sync::TryLockResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::TryLockResult`
#[cfg(not(feature = "std"))]
pub use crate::primitives::TryLockResult;

// ============================================================================
// Additional Foundation-Specific Types (no_std only)
// ============================================================================

// Note: The following types are only available in no_std mode as they are
// foundation_nostd-specific implementations without direct std equivalents.

#[cfg(not(feature = "std"))]
pub use crate::primitives::{
    AtomicCell, AtomicFlag, AtomicLazy, AtomicOption, RawSpinMutex, RawSpinMutexGuard,
    RawSpinRwLock, ReaderSpinRwLock, SpinMutex, SpinRwLock, SpinWait,
};
