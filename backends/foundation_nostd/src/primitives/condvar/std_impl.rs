//! Standard library implementation - direct re-exports from std.
//!
//! When std is available, we simply use the standard library's battle-tested
//! implementations directly.

pub use std::sync::{Condvar as CondVar, Mutex as CondVarMutex, MutexGuard as CondVarMutexGuard};

// For API compatibility, provide type aliases for the non-poisoning variants
// (in std mode, they behave the same as the regular versions)
pub use std::sync::{
    Condvar as CondVarNonPoisoning, Mutex as RawCondVarMutex, MutexGuard as RawCondVarMutexGuard,
};

// RwLockCondVar is just a regular Condvar in std mode
pub use std::sync::Condvar as RwLockCondVar;
