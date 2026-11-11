//! `NamespacedStore` — `StateStore` wrapper with automatic key prefixing.
//!
//! WHY: Deployables need isolated state so one deployable's resources can't
//! collide with another's. The wrapper makes prefixing structural, not convention.
//!
//! WHAT: Thin wrapper around any `StateStore` that prefixes all keys with a namespace.
//!
//! HOW: All key-based operations prepend `self.prefix`. `list()` filters by prefix
//! and strips it from returned keys.

use std::sync::Arc;

use foundation_context::Ctx;
use futures_lite::StreamExt;

use super::traits::{StateStore, StateStoreStream};
use super::types::{ResourceState, StateStatus};
use crate::core::errors::{StorageError, StorageResult};

/// `StateStore` wrapper that automatically prefixes all keys with a namespace.
///
/// WHY: Convention-based prefixing is easy to forget and impossible to enforce.
/// The wrapper makes it structural — once you have a `NamespacedStore`, every
/// operation is scoped. You can't accidentally write to another deployable's namespace.
///
/// WHAT: Implements `StateStore` by prepending `{prefix}/` to all keys.
///
/// HOW: `store(key, val)` → inner.store("{prefix}{key}", val), etc.
/// `list()` filters to prefix keys and strips the prefix from returned keys.
pub struct NamespacedStore<S: StateStore> {
    inner: Arc<S>,
    prefix: String,
}

impl<S: StateStore> NamespacedStore<S> {
    /// Create a new `NamespacedStore` wrapping the given `StateStore`.
    ///
    /// All operations on the returned store will be prefixed with `namespace`.
    #[must_use]
    pub fn new(inner: Arc<S>, namespace: &str) -> Self {
        Self {
            inner,
            prefix: format!("{namespace}/"),
        }
    }

    /// Get the namespace prefix used by this store.
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    fn full_key(&self, key: &str) -> String {
        format!("{prefix}{key}", prefix = self.prefix)
    }

    /// Store a typed value under the given key.
    ///
    /// Serializes the value as JSON into the `ResourceState`'s `output` field.
    /// `created_at` and `updated_at` come from the `Ctx` clock, never from a global
    /// ctx, so a deterministic simulation test controls them. A wall time
    /// outside the range of `chrono::DateTime<Utc>` falls back to
    /// `DateTime::UNIX_EPOCH`.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails or the inner store fails.
    pub async fn store_typed<T: serde::Serialize + Sync>(
        &self,
        key: &str,
        value: &T,
        ctx: &Ctx,
    ) -> StorageResult<()> {
        let full_key = self.full_key(key);
        let json_value = serde_json::to_value(value)
            .map_err(|e| StorageError::Serialization(format!("Failed to serialize state: {e}")))?;

        let now_ms = foundation_compact::time::unix_millis(ctx.clock().wall_now());
        let now =
            chrono::DateTime::from_timestamp_millis(now_ms).unwrap_or(chrono::DateTime::UNIX_EPOCH);
        let state = ResourceState {
            id: full_key.clone(),
            kind: String::new(),
            provider: String::new(),
            status: StateStatus::Created,
            environment: None,
            config_hash: String::new(),
            output: json_value,
            config_snapshot: serde_json::json!(null),
            created_at: now,
            updated_at: now,
        };
        self.inner.set(&full_key, &state).await
    }

    /// Get a typed value from the store.
    ///
    /// Deserializes the `ResourceState`'s `output` field into the requested type.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization fails or the inner store fails.
    pub async fn get_typed<T: for<'de> serde::Deserialize<'de>>(
        &self,
        key: &str,
    ) -> StorageResult<Option<T>> {
        match self.inner.get(&self.full_key(key)).await? {
            Some(state) => serde_json::from_value(state.output).map(Some).map_err(|e| {
                StorageError::Deserialization(format!("Failed to deserialize state: {e}"))
            }),
            None => Ok(None),
        }
    }

    /// Delete a key from the store.
    ///
    /// # Errors
    ///
    /// Returns an error if the inner store fails.
    pub async fn remove(&self, key: &str) -> StorageResult<()> {
        self.inner.delete(&self.full_key(key)).await
    }
}

#[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
impl<S: StateStore> StateStore for NamespacedStore<S> {
    async fn init(&self) -> StorageResult<()> {
        self.inner.init().await
    }

    async fn list(&self) -> StorageResult<StateStoreStream<String>> {
        let stream = self.inner.list_by_prefix(&self.prefix).await?;
        let prefix = self.prefix.clone();
        Ok(Box::pin(stream.map(move |item| {
            item.map(|id| id.strip_prefix(&prefix).map_or(id.clone(), str::to_string))
        })))
    }

    async fn count(&self) -> StorageResult<usize> {
        self.inner.count_by_prefix(&self.prefix).await
    }

    async fn get(&self, resource_id: &str) -> StorageResult<Option<ResourceState>> {
        self.inner.get(&self.full_key(resource_id)).await
    }

    async fn get_batch(&self, ids: &[&str]) -> StorageResult<StateStoreStream<ResourceState>> {
        let full_ids: Vec<String> = ids.iter().map(|id| self.full_key(id)).collect();
        let full_id_refs: Vec<&str> = full_ids.iter().map(String::as_str).collect();
        self.inner.get_batch(&full_id_refs).await
    }

    async fn all(&self) -> StorageResult<StateStoreStream<ResourceState>> {
        self.inner.all_by_prefix(&self.prefix).await
    }

    async fn set_if_version(
        &self,
        resource_id: &str,
        state: &ResourceState,
        expected: Option<chrono::DateTime<chrono::Utc>>,
    ) -> StorageResult<bool> {
        self.inner
            .set_if_version(&self.full_key(resource_id), state, expected)
            .await
    }

    async fn set(&self, resource_id: &str, state: &ResourceState) -> StorageResult<()> {
        // The inner store keys a write by `resource_id` and sets the stored
        // state's id from it, so passing the full namespaced key keeps the
        // write inside the namespace.
        self.inner.set(&self.full_key(resource_id), state).await
    }

    async fn delete(&self, resource_id: &str) -> StorageResult<()> {
        self.inner.delete(&self.full_key(resource_id)).await
    }

    async fn sync_remote(&self) -> StorageResult<()> {
        self.inner.sync_remote().await
    }

    async fn list_by_prefix(&self, prefix: &str) -> StorageResult<StateStoreStream<String>> {
        let stream = self.inner.list_by_prefix(&self.full_key(prefix)).await?;
        let ns_prefix = self.prefix.clone();
        Ok(Box::pin(stream.map(move |item| {
            item.map(|id| {
                id.strip_prefix(&ns_prefix)
                    .map_or(id.clone(), str::to_string)
            })
        })))
    }

    async fn count_by_prefix(&self, prefix: &str) -> StorageResult<usize> {
        self.inner.count_by_prefix(&self.full_key(prefix)).await
    }

    async fn all_by_prefix(&self, prefix: &str) -> StorageResult<StateStoreStream<ResourceState>> {
        self.inner.all_by_prefix(&self.full_key(prefix)).await
    }

    async fn find_by_kind(&self, kind: &str) -> StorageResult<StateStoreStream<ResourceState>> {
        let kind = kind.to_string();
        let all = self.inner.all_by_prefix(&self.prefix).await?;
        Ok(Box::pin(all.filter(move |item| {
            item.as_ref().map_or(true, |s| s.kind == kind)
        })))
    }

    async fn find_by_status(&self, status: &str) -> StorageResult<StateStoreStream<ResourceState>> {
        let status = status.to_string();
        let all = self.inner.all_by_prefix(&self.prefix).await?;
        Ok(Box::pin(all.filter(move |item| {
            item.as_ref()
                .map_or(true, |s| s.status.to_string() == status)
        })))
    }

    async fn find_by_provider(
        &self,
        provider: &str,
    ) -> StorageResult<StateStoreStream<ResourceState>> {
        let provider = provider.to_string();
        let all = self.inner.all_by_prefix(&self.prefix).await?;
        Ok(Box::pin(all.filter(move |item| {
            item.as_ref().map_or(true, |s| s.provider == provider)
        })))
    }

    async fn delete_by_prefix(&self, prefix: &str) -> StorageResult<usize> {
        self.inner.delete_by_prefix(&self.full_key(prefix)).await
    }
}
