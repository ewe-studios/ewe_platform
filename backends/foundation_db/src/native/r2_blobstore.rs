//! Cloudflare R2 storage backend (unified).
//!
//! WHY: R2 is Cloudflare's S3-compatible object storage - ideal for binary blobs
//! and JSON resource state.
//!
//! WHAT: `R2Store` implements `BlobStore` and `StateStore` traits for Cloudflare R2.
//!
//! HOW: Different constructors for different usage modes:
//!   - `R2Store::new_blob()` — BlobStore (raw binary, key: `{prefix}/key`)
//!   - `R2Store::new_state()` — StateStore (JSON objects, key: `{project}/{stage}/{id}.json`)

use foundation_core::valtron::{Stream, ThreadedValue};
use foundation_netio::simple_http::client::shared::body_reader::{AsyncSendSafeBody, collect_bytes_async, collect_string_async};
use foundation_netio::simple_http::client::SimpleHttpClient;
use foundation_netio::simple_http::shared::{SendSafeBody, SimpleHeader, Status};

use crate::core::errors::{StorageError, StorageResult};
use crate::core::state::traits::{StateStore, StateStoreStream};
use crate::core::state::types::ResourceState;
use crate::core::storage_provider::{
    AsyncBlobStore, BlobStore, StorageItemStream,
};

/// Default Cloudflare API base. Tests override via `R2Store::with_base_url`.
pub const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

// ---- Mode enum ----

#[derive(Clone)]
enum R2Mode {
    /// BlobStore. Keys prefixed with `{prefix}/` (slashes replaced by colons).
    Blob { bucket: String, prefix: String },
    /// StateStore. Keys prefixed with `{project}/{stage}/`, stored as `{id}.json`.
    State { bucket: String, project: String, stage: String },
}

// ---- R2Store ----

#[derive(Clone)]
pub struct R2Store {
    api_token: String,
    account_id: String,
    base_url: String,
    client: SimpleHttpClient,
    mode: R2Mode,
}

impl R2Store {
    // ========== Blob-mode constructors ==========

    /// BlobStore at production Cloudflare API.
    #[must_use]
    pub fn new_blob(api_token: &str, account_id: &str, bucket_name: &str, prefix: &str) -> Self {
        Self::new_blob_with_base_url(api_token, account_id, bucket_name, prefix, CF_API_BASE)
    }

    /// BlobStore with custom base URL (for tests).
    #[must_use]
    pub fn new_blob_with_base_url(
        api_token: &str, account_id: &str, bucket_name: &str, prefix: &str, base_url: &str,
    ) -> Self {
        Self {
            api_token: api_token.to_string(),
            account_id: account_id.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            client: SimpleHttpClient::from_system(),
            mode: R2Mode::Blob { bucket: bucket_name.to_string(), prefix: prefix.to_string() },
        }
    }

    /// BlobStore from environment.
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

    /// StateStore at production Cloudflare API.
    #[must_use]
    pub fn new_state(api_token: &str, account_id: &str, bucket_name: &str, project: &str, stage: &str) -> Self {
        Self::new_state_with_base_url(api_token, account_id, bucket_name, project, stage, CF_API_BASE)
    }

    /// StateStore with custom base URL (for tests).
    #[must_use]
    pub fn new_state_with_base_url(
        api_token: &str, account_id: &str, bucket_name: &str, project: &str, stage: &str, base_url: &str,
    ) -> Self {
        Self {
            api_token: api_token.to_string(),
            account_id: account_id.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            client: SimpleHttpClient::from_system(),
            mode: R2Mode::State { bucket: bucket_name.to_string(), project: project.to_string(), stage: stage.to_string() },
        }
    }

    /// StateStore from environment.
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
            R2Mode::Blob { prefix, .. } => format!("{}/{}", prefix, safe_key),
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
        format!("{}/accounts/{}/r2/buckets/{}/objects/{key}", base_url, account_id, bucket)
    }

    fn body_bytes(body: &SendSafeBody) -> Option<Vec<u8>> {
        match body {
            SendSafeBody::Text(s) => Some(s.as_bytes().to_vec()),
            SendSafeBody::Bytes(b) => Some(b.clone()),
            _ => None,
        }
    }

    fn body_text(body: &SendSafeBody) -> Option<String> {
        match body {
            SendSafeBody::Text(s) => Some(s.clone()),
            SendSafeBody::Bytes(b) => String::from_utf8(b.clone()).ok(),
            _ => None,
        }
    }


    fn wrap_value_state<T: Send + 'static>(val: T) -> StateStoreStream<T> {
        Box::new(std::iter::once(ThreadedValue::Value(Ok(val))))
    }

    fn wrap_vec_state<T: Send + 'static>(vals: Vec<T>) -> StateStoreStream<T> {
        Box::new(vals.into_iter().map(|v| ThreadedValue::Value(Ok(v))))
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
            .get(&url)
            .map_err(|e| StorageError::Backend(format!("R2 GET request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send_async()
            .await
            .map_err(|e| StorageError::Backend(format!("R2 GET request failed: {e}")))?;

        if response.get_status() == Status::NotFound {
            return Ok(None);
        }
        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!("R2 GET failed with status {}", response.get_status())));
        }
        let (_, _, body, ..) = response.into_parts();
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
            .put(&url)
            .map_err(|e| StorageError::Backend(format!("R2 PUT request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .header(SimpleHeader::CONTENT_TYPE, content_type)
            .body_bytes(data.to_vec())
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send_async()
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
            .delete(&url)
            .map_err(|e| StorageError::Backend(format!("R2 DELETE request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .header(SimpleHeader::CONNECTION, "close")
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send_async()
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
            .head(&url)
            .map_err(|e| StorageError::Backend(format!("R2 HEAD request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .header(SimpleHeader::CONNECTION, "close")
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send_async()
            .await
            .map_err(|e| StorageError::Backend(format!("R2 HEAD request failed: {e}")))?;

        Ok(response.get_status() == Status::OK)
    }

    /// Async LIST — returns parsed JSON listing response.
    #[allow(dead_code)]
    async fn list_objects_async(&self, prefix: &str) -> Result<serde_json::Value, StorageError> {
        let url = format!(
            "{}/accounts/{}/r2/buckets/{}/objects?prefix={}",
            self.base_url, self.account_id, self.bucket(), prefix
        );
        let response = self
            .client
            .get(&url)
            .map_err(|e| StorageError::Backend(format!("R2 LIST request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send_async()
            .await
            .map_err(|e| StorageError::Backend(format!("R2 LIST request failed: {e}")))?;

        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!("R2 LIST failed with status {}", response.get_status())));
        }
        let (_, _, body, ..) = response.into_parts();
        let text = collect_string_async(AsyncSendSafeBody::from(body))
            .await
            .map_err(|e| StorageError::Backend(format!("R2 body read failed: {e}")))?;
        serde_json::from_str(&text)
            .map_err(|e| StorageError::Serialization(format!("R2 LIST parse failed: {e}")))
    }
}

// ===========================================================================
// BlobStore
// ===========================================================================

impl BlobStore for R2Store {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let object_key = self.blob_object_key(key);
        let url = self.object_url(&object_key);
        let response = self
            .client
            .put(&url)
            .map_err(|e| StorageError::Backend(format!("R2 PUT request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .header(SimpleHeader::CONTENT_TYPE, "application/octet-stream")
            .body_bytes(data.to_vec())
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 PUT request failed: {e}")))?;
        let status_code: usize = response.get_status().into();
        if status_code >= 400 {
            return Err(StorageError::Backend(format!("R2 PUT failed with status {}", response.get_status())));
        }
        Ok(())
    }

    fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let object_key = self.blob_object_key(key);
        let url = self.object_url(&object_key);
        let response = self
            .client
            .get(&url)
            .map_err(|e| StorageError::Backend(format!("R2 GET request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 GET request failed: {e}")))?;
        if response.get_status() == Status::NotFound {
            return Ok(None);
        }
        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!("R2 GET failed with status {}", response.get_status())));
        }
        let bytes = Self::body_bytes(response.get_body_ref())
            .ok_or_else(|| StorageError::Backend("R2 GET: empty response body".to_string()))?;
        Ok(Some(bytes))
    }

    fn delete_blob(&self, key: &str) -> StorageResult<()> {
        let object_key = self.blob_object_key(key);
        let url = self.object_url(&object_key);
        let response = self
            .client
            .delete(&url)
            .map_err(|e| StorageError::Backend(format!("R2 DELETE request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .header(SimpleHeader::CONNECTION, "close")
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 DELETE request failed: {e}")))?;
        let status_code: usize = response.get_status().into();
        if status_code >= 400 && response.get_status() != Status::NotFound {
            return Err(StorageError::Backend(format!("R2 DELETE failed with status {}", response.get_status())));
        }
        Ok(())
    }

    fn blob_exists(&self, key: &str) -> StorageResult<bool> {
        let object_key = self.blob_object_key(key);
        let url = self.object_url(&object_key);
        let response = self
            .client
            .head(&url)
            .map_err(|e| StorageError::Backend(format!("R2 HEAD request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .header(SimpleHeader::CONNECTION, "close")
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 HEAD request failed: {e}")))?;
        Ok(response.get_status() == Status::OK)
    }
}

// ===========================================================================
// AsyncBlobStore
// ===========================================================================

#[async_trait::async_trait(?Send)]
impl AsyncBlobStore for R2Store {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        self.put_object_async(&self.blob_object_key(key), data, "application/octet-stream").await
    }

    async fn get_blob_async(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        self.get_object_async(&self.blob_object_key(key)).await
    }

    async fn delete_blob_async(&self, key: &str) -> StorageResult<()> {
        self.delete_object_async(&self.blob_object_key(key)).await
    }

    async fn blob_exists_async(&self, key: &str) -> StorageResult<bool> {
        self.head_object_async(&self.blob_object_key(key)).await
    }
}

// ===========================================================================
// StateStore
// ===========================================================================

impl StateStore for R2Store {
    fn init(&self) -> Result<(), StorageError> {
        // R2 buckets are pre-created — nothing to initialize.
        Ok(())
    }

    fn get(&self, resource_id: &str) -> Result<StateStoreStream<Option<ResourceState>>, StorageError> {
        let key = self.state_object_key(resource_id);
        let url = self.object_url(&key);
        let response = self
            .client
            .get(&url)
            .map_err(|e| StorageError::Backend(format!("R2 GET request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 GET request failed: {e}")))?;
        if response.get_status() == Status::NotFound {
            return Ok(Self::wrap_value_state(None));
        }
        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!("R2 GET failed with status {}", response.get_status())));
        }
        let text = Self::body_text(response.get_body_ref())
            .ok_or_else(|| StorageError::Backend("R2 GET: empty response body".to_string()))?;
        let state: ResourceState = serde_json::from_str(&text)
            .map_err(|e| StorageError::Serialization(format!("R2 GET parse failed: {e}")))?;
        Ok(Self::wrap_value_state(Some(state)))
    }

    fn set(&self, _resource_id: &str, state: &ResourceState) -> Result<StateStoreStream<()>, StorageError> {
        let key = self.state_object_key(&state.id);
        let url = self.object_url(&key);
        let body = serde_json::to_string_pretty(state)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let response = self
            .client
            .put(&url)
            .map_err(|e| StorageError::Backend(format!("R2 PUT request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .header(SimpleHeader::CONTENT_TYPE, "application/json")
            .body_text(body)
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 PUT request failed: {e}")))?;
        let status_code: usize = response.get_status().into();
        if status_code >= 400 {
            return Err(StorageError::Backend(format!("R2 PUT failed with status {}", response.get_status())));
        }
        Ok(Self::wrap_value_state(()))
    }

    fn delete(&self, resource_id: &str) -> Result<StateStoreStream<()>, StorageError> {
        let key = self.state_object_key(resource_id);
        let url = self.object_url(&key);
        let response = self
            .client
            .delete(&url)
            .map_err(|e| StorageError::Backend(format!("R2 DELETE request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 DELETE request failed: {e}")))?;
        let status_code: usize = response.get_status().into();
        if status_code >= 400 && response.get_status() != Status::NotFound {
            return Err(StorageError::Backend(format!("R2 DELETE failed with status {}", response.get_status())));
        }
        Ok(Self::wrap_value_state(()))
    }

    fn list(&self) -> Result<StateStoreStream<String>, StorageError> {
        let prefix = self.state_prefix();
        let url = format!(
            "{}/accounts/{}/r2/buckets/{}/objects?prefix={}",
            self.base_url, self.account_id, self.bucket(), prefix
        );
        let response = self
            .client
            .get(&url)
            .map_err(|e| StorageError::Backend(format!("R2 LIST request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 LIST request failed: {e}")))?;
        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!("R2 LIST failed with status {}", response.get_status())));
        }
        let text = Self::body_text(response.get_body_ref())
            .ok_or_else(|| StorageError::Backend("R2 LIST: empty response body".to_string()))?;
        let parsed: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| StorageError::Serialization(format!("R2 LIST parse failed: {e}")))?;
        let mut ids = Vec::new();
        if let Some(objects) = parsed.pointer("/result/objects").and_then(serde_json::Value::as_array) {
            for obj in objects {
                if let Some(key) = obj.get("key").and_then(serde_json::Value::as_str) {
                    if let Some(stripped) = key.strip_prefix(&prefix) {
                        if let Some(id_part) = stripped.strip_suffix(".json") {
                            ids.push(id_part.replace(':', "/"));
                        }
                    }
                }
            }
        }
        ids.sort();
        Ok(Self::wrap_vec_state(ids))
    }

    fn count(&self) -> Result<StateStoreStream<usize>, StorageError> {
        let list_stream = self.list()?;
        let mut count = 0usize;
        for item in list_stream {
            match item {
                ThreadedValue::Value(Ok(_)) => count += 1,
                ThreadedValue::Value(Err(e)) => return Err(e),
                ThreadedValue::Waiting => {}
            }
        }
        Ok(Self::wrap_value_state(count))
    }

    fn get_batch(&self, ids: &[&str]) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let mut results = Vec::new();
        for id in ids {
            let stream = self.get(id)?;
            for item in stream {
                match item {
                    ThreadedValue::Value(Ok(Some(state))) => results.push(state),
                    ThreadedValue::Value(Ok(None)) => {}
                    ThreadedValue::Value(Err(e)) => return Err(e),
                    ThreadedValue::Waiting => {}
                }
            }
        }
        Ok(Self::wrap_vec_state(results))
    }

    fn all(&self) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let list_stream = self.list()?;
        let mut ids = Vec::new();
        for item in list_stream {
            match item {
                ThreadedValue::Value(Ok(id)) => ids.push(id),
                ThreadedValue::Value(Err(e)) => return Err(e),
                ThreadedValue::Waiting => {}
            }
        }
        let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        self.get_batch(&id_refs)
    }

    fn list_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<String>, StorageError> {
        let store_prefix = self.state_prefix();
        let url = format!(
            "{}/accounts/{}/r2/buckets/{}/objects?prefix={}{}",
            self.base_url, self.account_id, self.bucket(), store_prefix, prefix
        );
        let response = self
            .client
            .get(&url)
            .map_err(|e| StorageError::Backend(format!("R2 LIST request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 LIST request failed: {e}")))?;
        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!("R2 LIST failed with status {}", response.get_status())));
        }
        let text = Self::body_text(response.get_body_ref())
            .ok_or_else(|| StorageError::Backend("R2 LIST: empty response body".to_string()))?;
        let parsed: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| StorageError::Serialization(format!("R2 LIST parse failed: {e}")))?;
        let mut ids = Vec::new();
        if let Some(objects) = parsed.pointer("/result/objects").and_then(serde_json::Value::as_array) {
            for obj in objects {
                if let Some(key) = obj.get("key").and_then(serde_json::Value::as_str) {
                    let rest = key.strip_prefix(&store_prefix).unwrap_or(key);
                    if let Some(id_part) = rest.strip_suffix(".json") {
                        ids.push(id_part.replace(':', "/"));
                    }
                }
            }
        }
        ids.sort();
        Ok(Self::wrap_vec_state(ids))
    }

    fn count_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<usize>, StorageError> {
        let stream = self.list_by_prefix(prefix)?;
        let mut count = 0usize;
        for item in stream {
            match item {
                ThreadedValue::Value(Ok(_)) => count += 1,
                ThreadedValue::Value(Err(e)) => return Err(e),
                ThreadedValue::Waiting => {}
            }
        }
        Ok(Self::wrap_value_state(count))
    }

    fn all_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let list_stream = self.list_by_prefix(prefix)?;
        let mut ids = Vec::new();
        for item in list_stream {
            match item {
                ThreadedValue::Value(Ok(id)) => ids.push(id),
                ThreadedValue::Value(Err(e)) => return Err(e),
                ThreadedValue::Waiting => {}
            }
        }
        let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        self.get_batch(&id_refs)
    }

    fn find_by_kind(&self, _kind: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        // R2 has no efficient way to filter by kind without scanning — return all and let caller filter.
        self.all()
    }

    fn find_by_status(&self, _status: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        // R2 has no efficient way to filter by status without scanning — return all and let caller filter.
        self.all()
    }

    fn find_by_provider(&self, _provider: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        // R2 has no efficient way to filter by provider without scanning — return all and let caller filter.
        self.all()
    }

    fn delete_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<usize>, StorageError> {
        let list_stream = self.list_by_prefix(prefix)?;
        let mut ids_to_delete = Vec::new();
        for item in list_stream {
            match item {
                ThreadedValue::Value(Ok(id)) => ids_to_delete.push(id),
                ThreadedValue::Value(Err(e)) => return Err(e),
                ThreadedValue::Waiting => {}
            }
        }
        for id in &ids_to_delete {
            for item in self.delete(id)? {
                if let ThreadedValue::Value(Err(e)) = item {
                    return Err(e);
                }
            }
        }
        Ok(Self::wrap_value_state(ids_to_delete.len()))
    }
}
