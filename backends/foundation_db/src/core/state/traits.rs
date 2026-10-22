//! The `StateStore` trait — persistence abstraction for deployment state.
//!
//! WHY: The deployment engine and providers need to read/write resource state
//! without knowing which storage backend is active.
//!
//! WHAT: An async trait for CRUD on `ResourceState`, plus listing, counting
//! and remote sync. Single values come back directly; multi-row results are
//! lazily pulled [`StateStoreStream`]s, never collected into memory.
//!
//! HOW: Every method is `async`, so a backend that does network I/O (libsql,
//! D1, R2) awaits it instead of blocking, and the same code runs natively and
//! on Cloudflare Workers. Callers `.await`; a sync entry point waits through
//! valtron's `block_on_future` / `.block()` behind `blocking_boundary!`.

use futures_lite::StreamExt;

use super::types::ResourceState;
use crate::core::errors::StorageResult;
use crate::core::storage_provider::AsyncStorageItemStream;

/// A lazily pulled stream of state results: `Send` natively, `!Send` on wasm,
/// like every async storage stream.
pub type StateStoreStream<T> = AsyncStorageItemStream<'static, T>;

/// Persistence backend for deployment resource state.
#[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
pub trait StateStore: Send + Sync {
    /// Initialize the store (create tables, directories, etc.).
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails (e.g. directory creation,
    /// table creation, connectivity check).
    async fn init(&self) -> StorageResult<()>;

    /// All resource IDs, as a stream.
    ///
    /// # Errors
    ///
    /// Returns an error if the query can't start; row errors come through the
    /// stream.
    async fn list(&self) -> StorageResult<StateStoreStream<String>>;

    /// Number of resources.
    ///
    /// # Errors
    ///
    /// Returns an error if the I/O fails.
    async fn count(&self) -> StorageResult<usize>;

    /// State for a single resource, `None` if absent.
    ///
    /// # Errors
    ///
    /// Returns an error if the I/O or decoding fails.
    async fn get(&self, resource_id: &str) -> StorageResult<Option<ResourceState>>;

    /// State for each of `ids` that exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the query can't start.
    async fn get_batch(&self, ids: &[&str]) -> StorageResult<StateStoreStream<ResourceState>>;

    /// Every resource state.
    ///
    /// # Errors
    ///
    /// Returns an error if the query can't start.
    async fn all(&self) -> StorageResult<StateStoreStream<ResourceState>>;

    /// Create or update a resource's state.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or I/O fails.
    async fn set(&self, resource_id: &str, state: &ResourceState) -> StorageResult<()>;

    /// Delete a resource's state (absent is not an error).
    ///
    /// # Errors
    ///
    /// Returns an error if the I/O fails.
    async fn delete(&self, resource_id: &str) -> StorageResult<()>;

    /// Sync state to a remote location. No-op for local-only backends (file,
    /// sqlite); libsql/Turso use it for explicit sync triggers.
    ///
    /// # Errors
    ///
    /// Returns an error if the sync fails.
    async fn sync_remote(&self) -> StorageResult<()> {
        Ok(())
    }

    /// Resource IDs starting with `prefix`.
    ///
    /// Default: filters [`list`](Self::list). SQL backends override with
    /// `WHERE id LIKE ?||'%'`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying operation fails.
    async fn list_by_prefix(&self, prefix: &str) -> StorageResult<StateStoreStream<String>> {
        let prefix = prefix.to_string();
        let all = self.list().await?;
        Ok(Box::pin(all.filter(move |item| {
            item.as_ref().map_or(true, |id| id.starts_with(&prefix))
        })))
    }

    /// Number of resources whose ID starts with `prefix`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying operation fails.
    async fn count_by_prefix(&self, prefix: &str) -> StorageResult<usize> {
        let mut ids = self.list_by_prefix(prefix).await?;
        let mut count = 0;
        while let Some(id) = ids.next().await {
            id?;
            count += 1;
        }
        Ok(count)
    }

    /// Resource states whose ID starts with `prefix`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying operation fails.
    async fn all_by_prefix(&self, prefix: &str) -> StorageResult<StateStoreStream<ResourceState>> {
        let prefix = prefix.to_string();
        let all = self.all().await?;
        Ok(Box::pin(all.filter(move |item| {
            item.as_ref().map_or(true, |state| state.id.starts_with(&prefix))
        })))
    }

    /// Resource states with the given `kind`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying operation fails.
    async fn find_by_kind(&self, kind: &str) -> StorageResult<StateStoreStream<ResourceState>> {
        let kind = kind.to_string();
        let all = self.all().await?;
        Ok(Box::pin(all.filter(move |item| {
            item.as_ref().map_or(true, |state| state.kind == kind)
        })))
    }

    /// Resource states with the given status (its string form).
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying operation fails.
    async fn find_by_status(&self, status: &str) -> StorageResult<StateStoreStream<ResourceState>> {
        let status = status.to_string();
        let all = self.all().await?;
        Ok(Box::pin(all.filter(move |item| {
            item.as_ref().map_or(true, |state| state.status.to_string() == status)
        })))
    }

    /// Resource states from the given provider.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying operation fails.
    async fn find_by_provider(&self, provider: &str) -> StorageResult<StateStoreStream<ResourceState>> {
        let provider = provider.to_string();
        let all = self.all().await?;
        Ok(Box::pin(all.filter(move |item| {
            item.as_ref().map_or(true, |state| state.provider == provider)
        })))
    }

    /// Delete every resource whose ID starts with `prefix`; returns how many.
    ///
    /// Default: lists the matching IDs, then deletes each. SQL backends
    /// override with `DELETE … WHERE id LIKE ?||'%'`.
    ///
    /// # Errors
    ///
    /// Returns an error if any underlying operation fails.
    async fn delete_by_prefix(&self, prefix: &str) -> StorageResult<usize> {
        // Collect the IDs first: deleting while a listing is open would mutate
        // what the listing reads.
        let mut ids = Vec::new();
        let mut stream = self.list_by_prefix(prefix).await?;
        while let Some(id) = stream.next().await {
            ids.push(id?);
        }
        for id in &ids {
            self.delete(id).await?;
        }
        Ok(ids.len())
    }
}
