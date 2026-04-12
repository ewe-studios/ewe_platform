//! Cloudflare R2 blob storage backend.
//!
//! WHY: R2 is Cloudflare's S3-compatible object storage - ideal for binary blobs.
//!
//! WHAT: `R2BlobStore` implements `BlobStore` trait for storing arbitrary binary data.
//! Uses Cloudflare's R2 API over HTTP with the `foundation_core` HTTP client.
//!
//! HOW: Each blob is stored as a raw binary object in an R2 bucket, keyed by
//! a configurable prefix + key pattern for namespacing.

use foundation_core::valtron::Stream;
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_core::wire::simple_http::{SendSafeBody, SimpleHeader, Status};

use crate::errors::{StorageError, StorageResult};
use crate::storage_provider::{BlobStore, StorageItemStream};

/// Default Cloudflare API base. Tests override this via [`R2BlobStore::with_base_url`].
pub const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Cloudflare R2 blob storage backend.
///
/// Stores binary data directly in R2 objects.
/// Uses the Cloudflare R2 API for PUT/GET/DELETE operations.
///
/// Object keys are prefixed with `{prefix}/` for namespacing.
pub struct R2BlobStore {
    api_token: String,
    account_id: String,
    bucket_name: String,
    prefix: String,
    base_url: String,
    client: SimpleHttpClient,
}

impl R2BlobStore {
    /// Create a new R2 blob store pointed at the production Cloudflare API.
    ///
    /// Object keys will be prefixed with `{prefix}/` for namespacing.
    #[must_use]
    pub fn new(api_token: &str, account_id: &str, bucket_name: &str, prefix: &str) -> Self {
        Self::with_base_url(api_token, account_id, bucket_name, prefix, CF_API_BASE)
    }

    /// Create a new R2 blob store with a custom API base URL.
    ///
    /// Used by integration tests to point the client at a local wrangler
    /// worker that emulates the Cloudflare R2 REST API.
    #[must_use]
    pub fn with_base_url(
        api_token: &str,
        account_id: &str,
        bucket_name: &str,
        prefix: &str,
        base_url: &str,
    ) -> Self {
        Self {
            api_token: api_token.to_string(),
            account_id: account_id.to_string(),
            bucket_name: bucket_name.to_string(),
            prefix: prefix.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            client: SimpleHttpClient::from_system(),
        }
    }

    /// Create from environment variables.
    ///
    /// - `DEPLOYMENT_R2_BUCKET` (required)
    /// - `CLOUDFLARE_API_TOKEN` (required)
    /// - `CLOUDFLARE_ACCOUNT_ID` (required)
    /// - `R2_BLOB_PREFIX` (optional, defaults to "blobs")
    ///
    /// # Errors
    ///
    /// Returns an error if required env vars are missing.
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
        Ok(Self::new(&token, &account, &bucket, &prefix))
    }

    fn object_key(&self, key: &str) -> String {
        // Sanitize key - replace problematic characters
        let safe_key = key.replace('/', ":");
        format!("{}/{}", self.prefix, safe_key)
    }

    fn object_url(&self, key: &str) -> String {
        format!(
            "{}/accounts/{}/r2/buckets/{}/objects/{key}",
            self.base_url, self.account_id, self.bucket_name
        )
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_token)
    }

    /// Extract response body as bytes from a `SendSafeBody`.
    fn body_bytes(body: &SendSafeBody) -> Option<Vec<u8>> {
        match body {
            SendSafeBody::Text(s) => Some(s.as_bytes().to_vec()),
            SendSafeBody::Bytes(b) => Some(b.clone()),
            _ => None,
        }
    }

    /// Wrap a single value into a `StorageItemStream`.
    fn wrap_value<T: Send + 'static>(val: T) -> StorageItemStream<'static, T> {
        Box::new(std::iter::once(Stream::Next(Ok(val))))
    }
}

impl BlobStore for R2BlobStore {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<StorageItemStream<'_, ()>> {
        let object_key = self.object_key(key);
        let url = self.object_url(&object_key);

        // R2 PUT with binary body
        let response = self
            .client
            .put(&url)
            .map_err(|e| StorageError::Backend(format!("R2 PUT request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header())
            .header(SimpleHeader::CONTENT_TYPE, "application/octet-stream")
            .body_bytes(data.to_vec())
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

    fn get_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, Option<Vec<u8>>>> {
        let object_key = self.object_key(key);
        let url = self.object_url(&object_key);

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

        let bytes = Self::body_bytes(response.get_body_ref())
            .ok_or_else(|| StorageError::Backend("R2 GET: empty response body".to_string()))?;

        Ok(Self::wrap_value(Some(bytes)))
    }

    fn delete_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let object_key = self.object_key(key);
        let url = self.object_url(&object_key);

        let response = self
            .client
            .delete(&url)
            .map_err(|e| StorageError::Backend(format!("R2 DELETE request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header())
            .header(SimpleHeader::CONNECTION, "close")
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 DELETE request failed: {e}")))?;

        let status_code: usize = response.get_status().into();
        // 404 is acceptable - object already deleted
        if status_code >= 400 && response.get_status() != Status::NotFound {
            return Err(StorageError::Backend(format!(
                "R2 DELETE failed with status {}",
                response.get_status()
            )));
        }

        Ok(Self::wrap_value(()))
    }

    fn blob_exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let object_key = self.object_key(key);
        let url = self.object_url(&object_key);

        // Use HEAD request to check existence without downloading
        let response = self
            .client
            .head(&url)
            .map_err(|e| StorageError::Backend(format!("R2 HEAD request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header())
            .header(SimpleHeader::CONNECTION, "close")
            .build_client()
            .map_err(|e| StorageError::Backend(format!("R2 request build failed: {e}")))?
            .send()
            .map_err(|e| StorageError::Backend(format!("R2 HEAD request failed: {e}")))?;

        let exists = response.get_status() == Status::OK;
        Ok(Self::wrap_value(exists))
    }
}
