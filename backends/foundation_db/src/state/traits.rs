//! The `StateStore` trait — persistence abstraction for deployment state.
//!
//! WHY: The deployment engine and providers need to read/write resource state
//! without knowing which storage backend is active.
//!
//! WHAT: A trait with methods for CRUD operations on `ResourceState`, plus
//! listing, counting, and remote sync. All I/O methods return
//! `StateStoreStream<T>` for lazy, composable evaluation.
//!
//! HOW: SQL backends produce streams from `run_future_iter` (worker thread
//! owns `!Send` rows). Sync backends produce streams from
//! `Vec::into_iter().map(...)`. Callers consume at their own boundary.

use foundation_core::valtron::ThreadedValue;

use crate::errors::StorageError;
use super::types::ResourceState;

/// Lazy stream of state store results.
///
/// SQL backends produce this from `run_future_iter` (channel-bridged worker
/// thread). Sync backends produce this from `Vec::into_iter().map(...)`.
/// Boxed because trait methods need a single return type across all backends.
pub type StateStoreStream<T> = Box<dyn Iterator<Item = ThreadedValue<T, StorageError>> + Send>;

/// Persistence backend for deployment resource state.
///
/// All I/O methods return `StateStoreStream<T>` for composability and lazy
/// evaluation. Callers consume streams at their own boundary — the store
/// never blocks internally (except `init()`, which is one-shot bootstrap).
pub trait StateStore: Send + Sync {
    /// Initialize the store (create tables, directories, etc.).
    ///
    /// This is the one exception to the stream pattern: it uses `exec_future`
    /// (one-shot bootstrap) and returns `Result` directly.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails (e.g. directory creation,
    /// table creation, connectivity check).
    fn init(&self) -> Result<(), StorageError>;

    /// List all resource IDs. Returns a lazy stream of IDs.
    ///
    /// # Errors
    ///
    /// Returns an error if scheduling or I/O fails.
    fn list(&self) -> Result<StateStoreStream<String>, StorageError>;

    /// Count resources. Returns a stream yielding a single `usize`.
    ///
    /// # Errors
    ///
    /// Returns an error if scheduling or I/O fails.
    fn count(&self) -> Result<StateStoreStream<usize>, StorageError>;

    /// Get state for a single resource.
    ///
    /// Stream yields one `Option<ResourceState>`: `Some` if found, `None` if not.
    ///
    /// # Errors
    ///
    /// Returns an error if scheduling or I/O fails.
    fn get(&self, resource_id: &str) -> Result<StateStoreStream<Option<ResourceState>>, StorageError>;

    /// Get state for multiple resources. Stream yields one `ResourceState` per match.
    ///
    /// # Errors
    ///
    /// Returns an error if scheduling or I/O fails.
    fn get_batch(&self, ids: &[&str]) -> Result<StateStoreStream<ResourceState>, StorageError>;

    /// Get all resource states. Stream yields one `ResourceState` per row.
    ///
    /// # Errors
    ///
    /// Returns an error if scheduling or I/O fails.
    fn all(&self) -> Result<StateStoreStream<ResourceState>, StorageError>;

    /// Set (create or update) state. Stream yields `()` on completion.
    ///
    /// # Errors
    ///
    /// Returns an error if scheduling, serialization, or I/O fails.
    fn set(&self, resource_id: &str, state: &ResourceState) -> Result<StateStoreStream<()>, StorageError>;

    /// Delete state. Stream yields `()` on completion.
    ///
    /// # Errors
    ///
    /// Returns an error if scheduling or I/O fails.
    fn delete(&self, resource_id: &str) -> Result<StateStoreStream<()>, StorageError>;

    /// Sync state to a remote location.
    ///
    /// No-op for local-only backends (file, sqlite). Used by libsql/Turso
    /// for explicit sync triggers.
    ///
    /// # Errors
    ///
    /// Returns an error if the sync operation fails.
    fn sync_remote(&self) -> Result<(), StorageError> {
        Ok(())
    }
}
