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

use super::types::ResourceState;
use crate::errors::StorageError;

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
    fn get(
        &self,
        resource_id: &str,
    ) -> Result<StateStoreStream<Option<ResourceState>>, StorageError>;

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
    fn set(
        &self,
        resource_id: &str,
        state: &ResourceState,
    ) -> Result<StateStoreStream<()>, StorageError>;

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

    /// List resource IDs matching the given key prefix.
    ///
    /// Default implementation fetches all IDs and filters in memory.
    /// SQL backends override with `WHERE id LIKE ?||'%'` for efficiency.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying operation fails.
    fn list_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<String>, StorageError> {
        let prefix = prefix.to_string();
        let all = self.list()?;
        Ok(Box::new(all.filter_map(move |item| match item {
            ThreadedValue::Value(Ok(id)) if id.starts_with(&prefix) => {
                Some(ThreadedValue::Value(Ok(id)))
            }
            ThreadedValue::Value(Ok(_)) => None,
            ThreadedValue::Value(Err(e)) => Some(ThreadedValue::Value(Err(e))),
        })))
    }

    /// Count resources matching the given key prefix.
    ///
    /// Default implementation calls `list_by_prefix` and counts.
    /// SQL backends override with `SELECT COUNT(*) WHERE id LIKE ?`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying operation fails.
    fn count_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<usize>, StorageError> {
        let prefix = prefix.to_string();
        let mut all = self.list()?;
        let count = all.try_fold(0usize, |n, item| match item {
            ThreadedValue::Value(Ok(id)) if id.starts_with(&prefix) => Ok(n + 1),
            ThreadedValue::Value(Ok(_)) => Ok(n),
            ThreadedValue::Value(Err(e)) => Err(e),
        });
        Ok(Box::new(std::iter::once(ThreadedValue::Value(count))))
    }

    /// Get all resource states matching the given key prefix.
    ///
    /// Default implementation fetches all rows and filters in memory.
    /// SQL backends override with `WHERE id LIKE ?||'%'`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying operation fails.
    fn all_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let prefix = prefix.to_string();
        let all = self.all()?;
        Ok(Box::new(all.filter_map(move |item| match item {
            ThreadedValue::Value(Ok(state)) if state.id.starts_with(&prefix) => {
                Some(ThreadedValue::Value(Ok(state)))
            }
            ThreadedValue::Value(Ok(_)) => None,
            ThreadedValue::Value(Err(e)) => Some(ThreadedValue::Value(Err(e))),
        })))
    }

    /// Find all resources with the given kind.
    ///
    /// Default implementation fetches all rows and filters by `kind`.
    /// SQL backends override with `WHERE kind = ?`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying operation fails.
    fn find_by_kind(&self, kind: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let kind = kind.to_string();
        let all = self.all()?;
        Ok(Box::new(all.filter_map(move |item| match item {
            ThreadedValue::Value(Ok(state)) if state.kind == kind => {
                Some(ThreadedValue::Value(Ok(state)))
            }
            ThreadedValue::Value(Ok(_)) => None,
            ThreadedValue::Value(Err(e)) => Some(ThreadedValue::Value(Err(e))),
        })))
    }

    /// Find all resources with the given status (string representation).
    ///
    /// Default implementation fetches all rows and filters by serialized status.
    /// SQL backends override with `WHERE status = ?`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying operation fails.
    fn find_by_status(
        &self,
        status: &str,
    ) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let status = status.to_string();
        let all = self.all()?;
        Ok(Box::new(all.filter_map(move |item| match item {
            ThreadedValue::Value(Ok(state)) => {
                let status_str = state.status.to_string();
                if status_str == status {
                    Some(ThreadedValue::Value(Ok(state)))
                } else {
                    None
                }
            }
            ThreadedValue::Value(Err(e)) => Some(ThreadedValue::Value(Err(e))),
        })))
    }

    /// Find all resources with the given provider.
    ///
    /// Default implementation fetches all rows and filters by `provider`.
    /// SQL backends override with `WHERE provider = ?`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying operation fails.
    fn find_by_provider(
        &self,
        provider: &str,
    ) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let provider = provider.to_string();
        let all = self.all()?;
        Ok(Box::new(all.filter_map(move |item| match item {
            ThreadedValue::Value(Ok(state)) if state.provider == provider => {
                Some(ThreadedValue::Value(Ok(state)))
            }
            ThreadedValue::Value(Ok(_)) => None,
            ThreadedValue::Value(Err(e)) => Some(ThreadedValue::Value(Err(e))),
        })))
    }

    /// Delete all resources matching the given key prefix.
    ///
    /// Returns a stream yielding the count of deleted items.
    /// Default implementation lists by prefix, deletes each individually.
    /// SQL backends override with `DELETE WHERE id LIKE ?||'%'`.
    ///
    /// # Errors
    ///
    /// Returns an error if any underlying operation fails.
    fn delete_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<usize>, StorageError> {
        let ids: Vec<String> = self
            .list_by_prefix(prefix)?
            .map(|item| match item {
                ThreadedValue::Value(Ok(id)) => Ok(id),
                ThreadedValue::Value(Err(e)) => Err(e),
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut count = 0;
        for id in &ids {
            let stream = self.delete(id)?;
            for item in stream {
                if let ThreadedValue::Value(Err(e)) = item {
                    return Err(e);
                }
            }
            count += 1;
        }
        Ok(Box::new(std::iter::once(ThreadedValue::Value(Ok(count)))))
    }
}
