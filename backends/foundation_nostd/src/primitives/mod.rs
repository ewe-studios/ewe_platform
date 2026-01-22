//! no_std-compatible synchronization primitives for WASM and embedded systems.

// Public modules
pub mod atomic_cell;
pub mod atomic_flag;
pub mod atomic_lazy;
pub mod atomic_option;
pub mod noop;
pub mod once;
pub mod once_lock;
pub mod poison;
pub mod raw_once;
pub mod spin_rwlock;
pub mod spin_wait;

// Re-export poison types
pub use poison::{LockResult, PoisonError, TryLockError, TryLockResult};

// Re-export rwlocks with poisoning
pub use spin_rwlock::{ReadGuard, SpinRwLock, WriteGuard};

// Re-export once types
pub use once::{Once, OnceState};
pub use once_lock::OnceLock;
pub use raw_once::RawOnce;

// Re-export atomic types
pub use atomic_cell::AtomicCell;
pub use atomic_flag::AtomicFlag;
pub use atomic_lazy::AtomicLazy;
pub use atomic_option::AtomicOption;

// Re-export synchronization helpers
pub use spin_wait::SpinWait;

// Platform-specific type aliases
// For single-threaded WASM, use no-op primitives
// For everything else (including WASM with atomics), use spin locks

/// Platform-appropriate RwLock type.
///
/// - On single-threaded WASM (no atomics): Uses `NoopRwLock`
/// - On all other platforms: Uses `SpinRwLock`
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
pub type RwLock<T> = noop::NoopRwLock<T>;

/// Platform-appropriate RwLock type.
///
/// - On single-threaded WASM (no atomics): Uses `NoopRwLock`
/// - On all other platforms: Uses `SpinRwLock`
#[cfg(any(not(target_arch = "wasm32"), target_feature = "atomics"))]
pub type RwLock<T> = SpinRwLock<T>;

/// Platform-appropriate Mutex type.
///
/// - On single-threaded WASM (no atomics): Uses `NoopMutex`
/// - On all other platforms: Note - SpinMutex not yet implemented, will use SpinRwLock as workaround
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
pub type Mutex<T> = noop::NoopMutex<T>;

/// Platform-appropriate Once type.
///
/// - On single-threaded WASM (no atomics): Uses `NoopOnce`
/// - On all other platforms: Uses `Once`
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
pub type PlatformOnce = noop::NoopOnce;

/// Platform-appropriate Once type.
///
/// - On single-threaded WASM (no atomics): Uses `NoopOnce`
/// - On all other platforms: Uses `Once`
#[cfg(any(not(target_arch = "wasm32"), target_feature = "atomics"))]
pub type PlatformOnce = Once;
