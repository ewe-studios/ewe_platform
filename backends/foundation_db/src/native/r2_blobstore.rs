//! Cloudflare R2 storage backend (unified).
//!
//! WHY: R2 is Cloudflare's S3-compatible object storage - ideal for binary blobs
//! and JSON resource state.
//!
//! WHAT: `R2Store` implements `BlobStore` and `StateStore` for Cloudflare R2.
//! Every operation is async; listings are paged through R2's cursor.
//!
//! HOW: Different constructors for different usage modes:
//!   - `R2Store::new_blob()` — `BlobStore` (raw binary, key: `{prefix}/key`)
//!   - `R2Store::new_state()` — `StateStore` (JSON objects, key: `{project}/{stage}/{id}.json`)

use futures_lite::{StreamExt, stream};
use foundation_netio::shared::client::body_reader::{AsyncSendSafeBody, collect_bytes_async, collect_string_async};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};
use foundation_netio::shared::http::{SimpleHeader, Status};

use crate::core::errors::{StorageError, StorageResult};
use crate::core::state::traits::{StateStore, StateStoreStream};
use crate::core::state::types::ResourceState;
use crate::core::storage_provider::BlobStore;

/// Default Cloudflare API base. Tests override via `R2Store::with_base_url`.
pub const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

// ---- Mode enum ----

#[derive(Clone)]
enum R2Mode {
    /// `BlobStore`. Keys prefixed with `{prefix}/` (slashes replaced by colons).
    Blob { bucket: String, prefix: String },
    /// `StateStore`. Keys prefixed with `{project}/{stage}/`, stored as `{id}.json`.
    State { bucket: String, project: String, stage: String },
}

// ---- R2Store ----

#[derive(Clone)]
pub struct R2Store {
    api_token: String,
    account_id: String,
    base_url: String,
    client: DynNetClient,
    mode: R2Mode,
}

impl R2Store {
    // ========== Blob-mode constructors ==========

    /// `BlobStore` at production Cloudflare API.
    #[must_use]
    pub fn new_blob(api_token: &str, account_id: &str, bucket_name: &str, prefix: &str) -> Self {
        Self::new_blob_with_base_url(api_token, account_id, bucket_name, prefix, CF_API_BASE)
    }

    /// `BlobStore` with custom base URL (for tests).
    #[must_use]
    pub fn new_blob_with_base_url(
        api_token: &str, account_id: &str, bucket_name: &str, prefix: &str, base_url: &str,
    ) -> Self {
        Self {
            api_token: api_token.to_string(),
            account_id: account_id.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            client: HttpClientBuilder::new().build(),
            mode: R2Mode::Blob { bucket: bucket_name.to_string(), prefix: prefix.to_string() },
        }
    }

    /// `BlobStore` from environment.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Connection`] if a required environment
    /// variable is unset.
    pub fn from_env() -> Result<Self, StorageError> {
        let bucket = std::env::var("DEPLOYMENT_R2_BUCKET").map_err(|_| {
            StorageError::Connection("DEPLOYMENT_R2_BUCKET must be set".to_string())
        })?;
        let token = std::env::var("CLOUDFLARE_API_TOKEN").map_err(|_| {
            StorageError::Connection("CLOUDFLARE_API_TOKEN must be set".to_string())
        })?;
        let account = std::env::var("CLOUDFLARE_ACCOUNT_ID").map_err(|_| {
            StorageError::Connection("CLOUDFLARE_ACCOUNT_ID must be set".to_string())
        })?;
        let prefix = std::env::var("R2_BLOB_PREFIX").unwrap_or_else(|_| "blobs".to_string());
        Ok(Self::new_blob(&token, &account, &bucket, &prefix))
    }

    // ========== State-mode constructors ==========

    /// `StateStore` at production Cloudflare API.
    #[must_use]
    pub fn new_state(api_token: &str, account_id: &str, bucket_name: &str, project: &str, stage: &str) -> Self {
        Self::new_state_with_base_url(api_token, account_id, bucket_name, project, stage, CF_API_BASE)
    }

    /// `StateStore` with custom base URL (for tests).
    #[must_use]
    pub fn new_state_with_base_url(
        api_token: &str, account_id: &str, bucket_name: &str, project: &str, stage: &str, base_url: &str,
    ) -> Self {
        Self {
            api_token: api_token.to_string(),
            account_id: account_id.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            client: HttpClientBuilder::new().build(),
            mode: R2Mode::State { bucket: bucket_name.to_string(), project: project.to_string(), stage: stage.to_string() },
        }
    }

    /// `StateStore` from environment.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Connection`] if a required environment
    /// variable is unset.
    pub fn from_env_state(project: &str, stage: &str) -> Result<Self, StorageError> {
        let bucket = std::env::var("DEPLOYMENT_R2_BUCKET").map_err(|_| {
            StorageError::Connection("DEPLOYMENT_R2_BUCKET must be set".to_string())
        })?;
        let token = std::env::var("CLOUDFLARE_API_TOKEN").map_err(|_| {
            StorageError::Connection("CLOUDFLARE_API_TOKEN must be set".to_string())
        })?;
        let account = std::env::var("CLOUDFLARE_ACCOUNT_ID").map_err(|_| {
            StorageError::Connection("CLOUDFLARE_ACCOUNT_ID must be set".to_string())
        })?;
        Ok(Self::new_state(&token, &account, &bucket, project, stage))
    }

    // ========== Shared helpers ==========

    fn auth_header_bearer(&self) -> String {
        format!("Bearer {}", self.api_token)
    }

    // Blob mode: key prefixed with `{prefix}/`, slashes -> colons
    fn blob_object_key(&self, key: &str) -> String {
        let safe_key = key.replace('/', ":");
        match &self.mode {
            R2Mode::Blob { prefix, .. } => format!("{prefix}/{safe_key}"),
            R2Mode::State { .. } => unreachable!("blob_object_key called on State mode"),
        }
    }

    // State mode: `{project}/{stage}/{id}.json`
    fn state_object_key(&self, resource_id: &str) -> String {
        let safe_id = resource_id.replace('/', ":");
        match &self.mode {
            R2Mode::State { project, stage, .. } => format!("{project}/{stage}/{safe_id}.json"),
            R2Mode::Blob { .. } => unreachable!("state_object_key called on Blob mode"),
        }
    }

    fn object_url(&self, key: &str) -> String {
        let (bucket, account_id, base_url) = match &self.mode {
            R2Mode::Blob { bucket, .. } | R2Mode::State { bucket, .. } => (bucket, &self.account_id, &self.base_url),
        };
        format!("{base_url}/accounts/{account_id}/r2/buckets/{bucket}/objects/{key}")
    }

    fn bucket(&self) -> &str {
        match &self.mode {
            R2Mode::Blob { bucket, .. } | R2Mode::State { bucket, .. } => bucket,
        }
    }

    fn state_prefix(&self) -> String {
        match &self.mode {
            R2Mode::State { project, stage, .. } => format!("{project}/{stage}/"),
            R2Mode::Blob { .. } => unreachable!("state_prefix called on Blob mode"),
        }
    }

    // ========== Async HTTP helpers ==========

    /// Async GET — returns object bytes, or None if not found.
    async fn get_object_async(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let url = self.object_url(key);
        let response = self
            .client
            .send_async(
                PreparedRequestBuilder::get(&url)
                    .map_err(|e| StorageError::Backend(format!("R2 GET request build failed: {e}")))?
                    .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
                    .build(),
            )
            .await
            .map_err(|e| StorageError::Backend(format!("R2 GET request failed: {e}")))?;

        if response.get_status() == Status::NotFound {
            return Ok(None);
        }
        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!("R2 GET failed with status {}", response.get_status())));
        }
        let (_, _, body) = response.into_parts();
        let bytes = collect_bytes_async(AsyncSendSafeBody::from(body))
            .await
            .map_err(|e| StorageError::Backend(format!("R2 body read failed: {e}")))?;
        Ok(Some(bytes))
    }

    /// Async PUT — stores data at the given key.
    async fn put_object_async(&self, key: &str, data: &[u8], content_type: &str) -> Result<(), StorageError> {
        let url = self.object_url(key);
        let response = self
            .client
            .send_async(
                PreparedRequestBuilder::put(&url)
                    .map_err(|e| StorageError::Backend(format!("R2 PUT request build failed: {e}")))?
                    .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
                    .header(SimpleHeader::CONTENT_TYPE, content_type)
                    .body_bytes(data.to_vec())
                    .build(),
            )
            .await
            .map_err(|e| StorageError::Backend(format!("R2 PUT request failed: {e}")))?;

        let status: usize = response.get_status().into();
        if status >= 400 {
            return Err(StorageError::Backend(format!("R2 PUT failed with status {}", response.get_status())));
        }
        Ok(())
    }

    /// Async DELETE — removes the object at the given key.
    async fn delete_object_async(&self, key: &str) -> Result<(), StorageError> {
        let url = self.object_url(key);
        let response = self
            .client
            .send_async(
                PreparedRequestBuilder::delete(&url)
                    .map_err(|e| StorageError::Backend(format!("R2 DELETE request build failed: {e}")))?
                    .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
                    .header(SimpleHeader::CONNECTION, "close")
                    .build(),
            )
            .await
            .map_err(|e| StorageError::Backend(format!("R2 DELETE request failed: {e}")))?;

        let status: usize = response.get_status().into();
        if status >= 400 && response.get_status() != Status::NotFound {
            return Err(StorageError::Backend(format!("R2 DELETE failed with status {}", response.get_status())));
        }
        Ok(())
    }

    /// Async HEAD — returns true if the object exists.
    async fn head_object_async(&self, key: &str) -> Result<bool, StorageError> {
        let url = self.object_url(key);
        let response = self
            .client
            .send_async(
                PreparedRequestBuilder::head(&url)
                    .map_err(|e| StorageError::Backend(format!("R2 HEAD request build failed: {e}")))?
                    .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
                    .header(SimpleHeader::CONNECTION, "close")
                    .build(),
            )
            .await
            .map_err(|e| StorageError::Backend(format!("R2 HEAD request failed: {e}")))?;

        Ok(response.get_status() == Status::OK)
    }

    /// Async LIST — one page of the object listing under `prefix`, starting
    /// at `cursor`. Returns the page's keys and the cursor of the next page
    /// (`None` on the last page).
    async fn list_objects_page(
        &self,
        prefix: &str,
        cursor: Option<&str>,
    ) -> Result<(Vec<String>, Option<String>), StorageError> {
        let mut url = format!(
            "{}/accounts/{}/r2/buckets/{}/objects?prefix={}",
            self.base_url,
            self.account_id,
            self.bucket(),
            query_escape(prefix)
        );
        if let Some(cursor) = cursor {
            url.push_str("&cursor=");
            url.push_str(&query_escape(cursor));
        }
        let response = self
            .client
            .send_async(
                PreparedRequestBuilder::get(&url)
                    .map_err(|e| StorageError::Backend(format!("R2 LIST request build failed: {e}")))?
                    .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
                    .build(),
            )
            .await
            .map_err(|e| StorageError::Backend(format!("R2 LIST request failed: {e}")))?;

        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!("R2 LIST failed with status {}", response.get_status())));
        }
        let (_, _, body) = response.into_parts();
        let text = collect_string_async(AsyncSendSafeBody::from(body))
            .await
            .map_err(|e| StorageError::Backend(format!("R2 body read failed: {e}")))?;
        let parsed: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| StorageError::Serialization(format!("R2 LIST parse failed: {e}")))?;

        // The Cloudflare API returns `result` as the object array; older
        // responses nest it under `result.objects`. Accept both.
        let objects = parsed
            .pointer("/result")
            .and_then(|r| r.as_array().or_else(|| r.get("objects").and_then(serde_json::Value::as_array)))
            .ok_or_else(|| StorageError::Serialization("R2 LIST: response has no object list".to_string()))?;
        let keys = objects
            .iter()
            .filter_map(|obj| obj.get("key").and_then(serde_json::Value::as_str).map(str::to_string))
            .collect();
        let truncated = parsed
            .pointer("/result_info/is_truncated")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let next = if truncated {
            let cursor = parsed
                .pointer("/result_info/cursor")
                .and_then(serde_json::Value::as_str)
                .filter(|c| !c.is_empty())
                .ok_or_else(|| StorageError::Serialization("R2 LIST: truncated page has no cursor".to_string()))?;
            Some(cursor.to_string())
        } else {
            None
        };
        Ok((keys, next))
    }

    /// State IDs whose ID starts with `id_prefix`, sorted, across every page
    /// of the listing. Only IDs are held, never the state objects.
    async fn state_ids(&self, id_prefix: &str) -> Result<Vec<String>, StorageError> {
        let store_prefix = self.state_prefix();
        let list_prefix = format!("{store_prefix}{}", id_prefix.replace('/', ":"));
        let mut ids = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let (keys, next) = self.list_objects_page(&list_prefix, cursor.as_deref()).await?;
            ids.extend(keys.iter().filter_map(|key| {
                key.strip_prefix(&store_prefix)
                    .and_then(|rest| rest.strip_suffix(".json"))
                    .map(|id| id.replace(':', "/"))
            }));
            match next {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        ids.sort();
        Ok(ids)
    }

    /// A stream that fetches each of `ids` as it is pulled, skipping IDs
    /// that no longer exist.
    fn states_for(&self, ids: Vec<String>) -> StateStoreStream<ResourceState> {
        let store = self.clone();
        Box::pin(
            stream::iter(ids)
                .then(move |id| {
                    let store = store.clone();
                    async move { store.get(&id).await }
                })
                .filter_map(Result::transpose),
        )
    }
}

/// Percent-encode a query-string value (everything but RFC 3986 unreserved
/// characters).
fn query_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            out.push('%');
            out.push(char::from(HEX[usize::from(byte >> 4)]));
            out.push(char::from(HEX[usize::from(byte & 0x0F)]));
        }
    }
    out
}

// ===========================================================================
// BlobStore
// ===========================================================================

#[async_trait::async_trait]
impl BlobStore for R2Store {
    async fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        self.put_object_async(&self.blob_object_key(key), data, "application/octet-stream").await
    }

    async fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        self.get_object_async(&self.blob_object_key(key)).await
    }

    async fn delete_blob(&self, key: &str) -> StorageResult<()> {
        self.delete_object_async(&self.blob_object_key(key)).await
    }

    async fn blob_exists(&self, key: &str) -> StorageResult<bool> {
        self.head_object_async(&self.blob_object_key(key)).await
    }
}

// ===========================================================================
// StateStore
// ===========================================================================

#[async_trait::async_trait]
impl StateStore for R2Store {
    async fn init(&self) -> StorageResult<()> {
        // R2 buckets are pre-created — nothing to initialize.
        Ok(())
    }

    async fn list(&self) -> StorageResult<StateStoreStream<String>> {
        self.list_by_prefix("").await
    }

    async fn count(&self) -> StorageResult<usize> {
        Ok(self.state_ids("").await?.len())
    }

    async fn get(&self, resource_id: &str) -> StorageResult<Option<ResourceState>> {
        let Some(bytes) = self.get_object_async(&self.state_object_key(resource_id)).await? else {
            return Ok(None);
        };
        let state = serde_json::from_slice(&bytes)
            .map_err(|e| StorageError::Serialization(format!("R2 GET parse failed: {e}")))?;
        Ok(Some(state))
    }

    async fn get_batch(&self, ids: &[&str]) -> StorageResult<StateStoreStream<ResourceState>> {
        Ok(self.states_for(ids.iter().map(|id| (*id).to_string()).collect()))
    }

    async fn all(&self) -> StorageResult<StateStoreStream<ResourceState>> {
        self.all_by_prefix("").await
    }

    async fn set(&self, _resource_id: &str, state: &ResourceState) -> StorageResult<()> {
        let body = serde_json::to_vec_pretty(state).map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put_object_async(&self.state_object_key(&state.id), &body, "application/json")
            .await
    }

    async fn delete(&self, resource_id: &str) -> StorageResult<()> {
        self.delete_object_async(&self.state_object_key(resource_id)).await
    }

    async fn list_by_prefix(&self, prefix: &str) -> StorageResult<StateStoreStream<String>> {
        let ids = self.state_ids(prefix).await?;
        Ok(Box::pin(stream::iter(ids.into_iter().map(Ok))))
    }

    async fn count_by_prefix(&self, prefix: &str) -> StorageResult<usize> {
        Ok(self.state_ids(prefix).await?.len())
    }

    async fn all_by_prefix(&self, prefix: &str) -> StorageResult<StateStoreStream<ResourceState>> {
        let ids = self.state_ids(prefix).await?;
        Ok(self.states_for(ids))
    }

    // `find_by_*` and `delete_by_prefix` use the trait defaults: R2 can't
    // filter on the server, so they scan `all()` and filter / list then delete.
}
