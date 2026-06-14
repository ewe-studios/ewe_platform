//! Deployment state management — trait, types, backends, and factory.
//!
//! WHY: The deployment engine needs persistent, backend-agnostic state to track
//! what's deployed, detect config changes, and coordinate rollbacks.
//!
//! WHAT: `StateStore` trait with six interchangeable backends (JSON files,
//! `SQLite`, libsql, Turso, Cloudflare R2, Cloudflare D1), plus helpers
//! and a factory for auto-detection.
//!
//! HOW: All backends implement the same `StateStore` trait. I/O methods return
//! `StateStoreStream<T>` (lazy iterators). The factory selects a backend
//! from environment variables.

pub mod file;
pub mod hash;
pub mod helpers;
pub mod namespaced;
pub mod resource_identifier;
pub mod store_state_task;
pub mod traits;
pub mod types;

// Re-exports for convenience
pub use file::FileStateStore;
pub use hash::config_hash;
pub use helpers::{collect_all, collect_first, drive_to_completion};
pub use traits::StateStore;
pub use types::{ResourceState, StateStatus};
