//! Cloudflare R2 object storage state store.
//!
//! WHY: Remote state storage for teams deploying to Cloudflare. State lives
//! in the same ecosystem, accessible from any machine with API credentials.
//!
//! WHAT: Each resource is a JSON object in an R2 bucket, keyed by
//! `{project}/{stage}/{resource_id}.json`. CRUD via the Cloudflare R2 API.
//!
//! HOW: Uses `SimpleHttpClient` (synchronous) for all HTTP calls. No Valtron
//! needed. Returns `StateStoreStream` via `Vec::into_iter().map(...)`.

use foundation_core::valtron::ThreadedValue;
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_core::wire::simple_http::{SendSafeBody, SimpleHeader, Status};

use super::traits::{StateStore, StateStoreStream};
use super::types::ResourceState;
use crate::errors::StorageError;

const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Cloudflare R2 object storage state store.
///
/// Stores each resource as a JSON object in an R2 bucket.
/// Uses the Cloudflare API (not S3-compatible endpoint) for CRUD.
///
/// Object keys are prefixed with `{project}/{stage}/` for namespacing.
pub struct R2StateStore {
    api_token: String,
    account_id: String,
    bucket_name: String,
    prefix: String,  // "{project}/{stage}/"
    client: SimpleHttpClient,
}

impl R2StateStore {
    /// Create a new R2 state store.
    ///
    /// Object keys will be prefixed with `{project}/{stage}/` for namespacing.
    #[must_use]
    pub fn new(api_token: &str, account_id: &str, bucket_name: &str, project: &str, stage: &str) -> Self {
        Self {
            api_token: api_token.to_string(),
            account_id: account_id.to_string(),
            bucket_name: bucket_name.to_string(),
            prefix: format!("{project}/{stage}/"),
            client: SimpleHttpClient::from_system(),
        }
    }

    /// Create from environment variables.
    ///
    /// - `DEPLOYMENT_R2_BUCKET` (required)
    /// - `CLOUDFLARE_API_TOKEN` (required)
    /// - `CLOUDFLARE_ACCOUNT_ID` (required)
    ///
    /// # Errors
    ///
    /// Returns an error if required env vars are missing.
    pub fn from_env(project: &str, stage: &str) -> Result<Self, StorageError> {
        let bucket = std::env::var("DEPLOYMENT_R2_BUCKET").map_err(|_| {
            StorageError::Connection("DEPLOYMENT_R2_BUCKET must be set".to_string())
        })?;
        let token = std::env::var("CLOUDFLARE_API_TOKEN").map_err(|_| {
            StorageError::Connection("CLOUDFLARE_API_TOKEN must be set".to_string())
        })?;
        let account = std::env::var("CLOUDFLARE_ACCOUNT_ID").map_err(|_| {
            StorageError::Connection("CLOUDFLARE_ACCOUNT_ID must be set".to_string())
        })?;
        Ok(Self::new(&token, &account, &bucket, project, stage))
    }

    fn object_key(&self, resource_id: &str) -> String {
        let safe_id = resource_id.replace('/', ":");
        format!("{}{safe_id}.json", self.prefix)
    }

    fn object_url(&self, key: &str) -> String {
        format!(
            "{CF_API_BASE}/accounts/{}/r2/buckets/{}/objects/{key}",
            self.account_id, self.bucket_name
        )
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_token)
    }

    /// Extract response body text from a `SendSafeBody`.
    fn body_text(body: &SendSafeBody) -> Option<String> {
        match body {
            SendSafeBody::Text(s) => Some(s.clone()),
            SendSafeBody::Bytes(b) => String::from_utf8(b.clone()).ok(),
            _ => None,
        }
    }

    /// Wrap a single value into a `StateStoreStream`.
    fn wrap_value<T: Send + 'static>(val: T) -> StateStoreStream<T> {
        Box::new(std::iter::once(ThreadedValue::Value(Ok(val))))
    }

    /// Wrap a `Vec` into a `StateStoreStream`.
    fn wrap_vec<T: Send + 'static>(vals: Vec<T>) -> StateStoreStream<T> {
        Box::new(vals.into_iter().map(|v| ThreadedValue::Value(Ok(v))))
    }
}

impl StateStore for R2StateStore {
    fn init(&self) -> Result<(), StorageError> {
        // R2 buckets are pre-created — nothing to initialize.
        Ok(())
    }

    fn get(
        &self,
        resource_id: &str,
    ) -> Result<StateStoreStream<Option<ResourceState>>, StorageError> {
        let key = self.object_key(resource_id);
        let url = self.object_url(&key);
        let response = self
            .client
            .get(&url)
            .map_err(|e| StorageError::Backend(format!("R2 GET request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header())
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 GET request failed: {e}")))?;

        if response.get_status() == Status::NotFound {
            return Ok(Self::wrap_value(None));
        }
        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!(
                "R2 GET failed with status {}",
                response.get_status()
            )));
        }

        let text = Self::body_text(response.get_body_ref())
            .ok_or_else(|| StorageError::Backend("R2 GET: empty response body".to_string()))?;
        let state: ResourceState = serde_json::from_str(&text)
            .map_err(|e| StorageError::Serialization(format!("R2 GET parse failed: {e}")))?;
        Ok(Self::wrap_value(Some(state)))
    }

    fn set(
        &self,
        _resource_id: &str,
        state: &ResourceState,
    ) -> Result<StateStoreStream<()>, StorageError> {
        let key = self.object_key(&state.id);
        let url = self.object_url(&key);
        let body = serde_json::to_string_pretty(state)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let response = self
            .client
            .put(&url)
            .map_err(|e| StorageError::Backend(format!("R2 PUT request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header())
            .header(SimpleHeader::CONTENT_TYPE, "application/json")
            .body_text(body)
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 PUT request failed: {e}")))?;

        let status_code: usize = response.get_status().into();
        if status_code >= 400 {
            return Err(StorageError::Backend(format!(
                "R2 PUT failed with status {}",
                response.get_status()
            )));
        }
        Ok(Self::wrap_value(()))
    }

    fn delete(&self, resource_id: &str) -> Result<StateStoreStream<()>, StorageError> {
        let key = self.object_key(resource_id);
        let url = self.object_url(&key);
        let response = self
            .client
            .delete(&url)
            .map_err(|e| StorageError::Backend(format!("R2 DELETE request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header())
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 DELETE request failed: {e}")))?;

        let status_code: usize = response.get_status().into();
        if status_code >= 400 && response.get_status() != Status::NotFound {
            return Err(StorageError::Backend(format!(
                "R2 DELETE failed with status {}",
                response.get_status()
            )));
        }
        Ok(Self::wrap_value(()))
    }

    fn list(&self) -> Result<StateStoreStream<String>, StorageError> {
        let url = format!(
            "{CF_API_BASE}/accounts/{}/r2/buckets/{}/objects?prefix={}",
            self.account_id, self.bucket_name, self.prefix
        );
        let response = self
            .client
            .get(&url)
            .map_err(|e| StorageError::Backend(format!("R2 LIST request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header())
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 LIST request failed: {e}")))?;

        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!(
                "R2 LIST failed with status {}",
                response.get_status()
            )));
        }

        let text = Self::body_text(response.get_body_ref())
            .ok_or_else(|| StorageError::Backend("R2 LIST: empty response body".to_string()))?;

        // Cloudflare R2 list response: { "result": { "objects": [{ "key": "..." }, ...] } }
        let parsed: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| StorageError::Serialization(format!("R2 LIST parse failed: {e}")))?;

        let mut ids = Vec::new();
        if let Some(objects) = parsed
            .pointer("/result/objects")
            .and_then(serde_json::Value::as_array)
        {
            for obj in objects {
                if let Some(key) = obj.get("key").and_then(serde_json::Value::as_str) {
                    // Strip prefix and .json suffix to get resource ID
                    if let Some(stripped) = key.strip_prefix(&self.prefix) {
                        if let Some(id_part) = stripped.strip_suffix(".json") {
                            ids.push(id_part.replace(':', "/"));
                        }
                    }
                }
            }
        }
        ids.sort();
        Ok(Self::wrap_vec(ids))
    }

    fn count(&self) -> Result<StateStoreStream<usize>, StorageError> {
        // List and count — R2 has no separate count API.
        let list_stream = self.list()?;
        let mut count = 0usize;
        for item in list_stream {
            match item {
                ThreadedValue::Value(Ok(_)) => count += 1,
                ThreadedValue::Value(Err(e)) => return Err(e),
            }
        }
        Ok(Self::wrap_value(count))
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
                }
            }
        }
        Ok(Self::wrap_vec(results))
    }

    fn all(&self) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let list_stream = self.list()?;
        let mut ids = Vec::new();
        for item in list_stream {
            match item {
                ThreadedValue::Value(Ok(id)) => ids.push(id),
                ThreadedValue::Value(Err(e)) => return Err(e),
            }
        }
        let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        self.get_batch(&id_refs)
    }
}
