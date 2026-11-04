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

use std::collections::VecDeque;

use foundation_cloudflare_types::R2ListObjectsResponse;
use foundation_netio::shared::client::body_reader::{
    collect_bytes_async, collect_string_async, AsyncSendSafeBody,
};
use foundation_netio::shared::http::pushable_body::pushable_request_body;
use foundation_netio::shared::http::{SimpleHeader, Status};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};
use futures_lite::{stream, StreamExt};

use crate::core::errors::{StorageError, StorageResult};
use crate::core::state::traits::{StateStore, StateStoreStream};
use crate::core::state::types::ResourceState;
use crate::core::storage_provider::{
    BlobListStream, BlobPutCondition, BlobRange, BlobStore, BlobStream,
};

/// Default Cloudflare API base (hosted). `R2Store::new_*` put
/// `/accounts/{account_id}` after it; the `*_with_base_url` constructors take
/// the whole prefix instead (see [`hosted_api_prefix`]).
pub const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// The API prefix of the hosted Cloudflare API for `account_id`. Pass it, or the
/// prefix of wrangler's local explorer API
/// (`http://127.0.0.1:<port>/cdn-cgi/local/explorer/api`, no account segment),
/// as the `base_url` of the `*_with_base_url` constructors.
#[must_use]
pub fn hosted_api_prefix(account_id: &str) -> String {
    format!("{CF_API_BASE}/accounts/{account_id}")
}

// ---- Mode enum ----

#[derive(Clone)]
enum R2Mode {
    /// `BlobStore`. Keys prefixed with `{prefix}/` (slashes replaced by colons).
    Blob { bucket: String, prefix: String },
    /// `StateStore`. Keys prefixed with `{project}/{stage}/`, stored as `{id}.json`.
    State {
        bucket: String,
        project: String,
        stage: String,
    },
}

// ---- R2Store ----

#[derive(Clone)]
pub struct R2Store {
    api_token: String,
    /// Everything before `/r2/buckets/...` in the request URL.
    api_prefix: String,
    client: DynNetClient,
    mode: R2Mode,
}

impl R2Store {
    // ========== Blob-mode constructors ==========

    /// `BlobStore` at production Cloudflare API.
    #[must_use]
    pub fn new_blob(api_token: &str, account_id: &str, bucket_name: &str, prefix: &str) -> Self {
        Self::new_blob_with_base_url(
            api_token,
            account_id,
            bucket_name,
            prefix,
            &hosted_api_prefix(account_id),
        )
    }

    /// `BlobStore` with a custom API prefix: everything before `/r2/buckets/...`.
    /// For the hosted API that is [`hosted_api_prefix`]; for wrangler's local
    /// explorer API it is `http://127.0.0.1:<port>/cdn-cgi/local/explorer/api`.
    /// `account_id` is not used in the URL then.
    #[must_use]
    pub fn new_blob_with_base_url(
        api_token: &str,
        _account_id: &str,
        bucket_name: &str,
        prefix: &str,
        base_url: &str,
    ) -> Self {
        Self {
            api_token: api_token.to_string(),
            api_prefix: base_url.trim_end_matches('/').to_string(),
            client: HttpClientBuilder::new().build(),
            mode: R2Mode::Blob {
                bucket: bucket_name.to_string(),
                prefix: prefix.to_string(),
            },
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
    pub fn new_state(
        api_token: &str,
        account_id: &str,
        bucket_name: &str,
        project: &str,
        stage: &str,
    ) -> Self {
        Self::new_state_with_base_url(
            api_token,
            account_id,
            bucket_name,
            project,
            stage,
            &hosted_api_prefix(account_id),
        )
    }

    /// `StateStore` with a custom API prefix (see
    /// [`new_blob_with_base_url`](Self::new_blob_with_base_url)).
    #[must_use]
    pub fn new_state_with_base_url(
        api_token: &str,
        _account_id: &str,
        bucket_name: &str,
        project: &str,
        stage: &str,
        base_url: &str,
    ) -> Self {
        Self {
            api_token: api_token.to_string(),
            api_prefix: base_url.trim_end_matches('/').to_string(),
            client: HttpClientBuilder::new().build(),
            mode: R2Mode::State {
                bucket: bucket_name.to_string(),
                project: project.to_string(),
                stage: stage.to_string(),
            },
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
        let bucket = match &self.mode {
            R2Mode::Blob { bucket, .. } | R2Mode::State { bucket, .. } => bucket,
        };
        // The key is one path segment: `/` inside it is `%2F` (the local explorer
        // API matches `objects/:key` as a single segment).
        format!(
            "{}/r2/buckets/{bucket}/objects/{}",
            self.api_prefix,
            query_escape(key)
        )
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
                    .map_err(|e| {
                        StorageError::Backend(format!("R2 GET request build failed: {e}"))
                    })?
                    .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
                    .build(),
            )
            .await
            .map_err(|e| StorageError::Backend(format!("R2 GET request failed: {e}")))?;

        if response.get_status() == Status::NotFound {
            return Ok(None);
        }
        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!(
                "R2 GET failed with status {}",
                response.get_status()
            )));
        }
        let (_, _, body) = response.into_async_parts();
        let bytes = collect_bytes_async(body)
            .await
            .map_err(|e| StorageError::Backend(format!("R2 body read failed: {e}")))?;
        Ok(Some(bytes))
    }

    /// Async PUT — stores data at the given key.
    async fn put_object_async(
        &self,
        key: &str,
        data: &[u8],
        content_type: &str,
    ) -> Result<(), StorageError> {
        let url = self.object_url(key);
        let response = self
            .client
            .send_async(
                PreparedRequestBuilder::put(&url)
                    .map_err(|e| {
                        StorageError::Backend(format!("R2 PUT request build failed: {e}"))
                    })?
                    .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
                    .header(SimpleHeader::CONTENT_TYPE, content_type)
                    .body_bytes(data.to_vec())
                    .build(),
            )
            .await
            .map_err(|e| StorageError::Backend(format!("R2 PUT request failed: {e}")))?;

        let status: usize = response.get_status().into();
        if status >= 400 {
            return Err(StorageError::Backend(format!(
                "R2 PUT failed with status {}",
                response.get_status()
            )));
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
                    .map_err(|e| {
                        StorageError::Backend(format!("R2 DELETE request build failed: {e}"))
                    })?
                    .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
                    .header(SimpleHeader::CONNECTION, "close")
                    .build(),
            )
            .await
            .map_err(|e| StorageError::Backend(format!("R2 DELETE request failed: {e}")))?;

        let status: usize = response.get_status().into();
        if status >= 400 && response.get_status() != Status::NotFound {
            return Err(StorageError::Backend(format!(
                "R2 DELETE failed with status {}",
                response.get_status()
            )));
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
                    .map_err(|e| {
                        StorageError::Backend(format!("R2 HEAD request build failed: {e}"))
                    })?
                    .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
                    .header(SimpleHeader::CONNECTION, "close")
                    .build(),
            )
            .await
            .map_err(|e| StorageError::Backend(format!("R2 HEAD request failed: {e}")))?;

        Ok(response.get_status() == Status::OK)
    }

    /// Async HEAD — returns the object's `etag` header, or `None` if missing.
    async fn head_object_etag_async(&self, key: &str) -> Result<Option<String>, StorageError> {
        let url = self.object_url(key);
        let response = self
            .client
            .send_async(
                PreparedRequestBuilder::head(&url)
                    .map_err(|e| {
                        StorageError::Backend(format!("R2 HEAD request build failed: {e}"))
                    })?
                    .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
                    .header(SimpleHeader::CONNECTION, "close")
                    .build(),
            )
            .await
            .map_err(|e| StorageError::Backend(format!("R2 HEAD request failed: {e}")))?;

        if response.get_status() != Status::OK {
            return Ok(None);
        }
        let (_, headers, _body) = response.into_async_parts();
        Ok(headers
            .get(&SimpleHeader::ETAG)
            .and_then(|values| values.first().cloned()))
    }

    /// Async GET — returns the object body as a byte-chunk stream.
    async fn get_object_stream_async(&self, key: &str) -> Result<Option<BlobStream>, StorageError> {
        let url = self.object_url(key);
        let response = self
            .client
            .send_async(
                PreparedRequestBuilder::get(&url)
                    .map_err(|e| {
                        StorageError::Backend(format!("R2 GET request build failed: {e}"))
                    })?
                    .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
                    .build(),
            )
            .await
            .map_err(|e| StorageError::Backend(format!("R2 GET request failed: {e}")))?;

        if response.get_status() == Status::NotFound {
            return Ok(None);
        }
        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!(
                "R2 GET failed with status {}",
                response.get_status()
            )));
        }
        let (_, _, body) = response.into_async_parts();
        Ok(Some(body_to_blob_stream(body)))
    }

    /// Async ranged GET — returns one byte range as a stream.
    async fn get_object_range_async(
        &self,
        key: &str,
        range: BlobRange,
    ) -> Result<BlobStream, StorageError> {
        let url = self.object_url(key);
        let response = self
            .client
            .send_async(
                PreparedRequestBuilder::get(&url)
                    .map_err(|e| {
                        StorageError::Backend(format!("R2 GET request build failed: {e}"))
                    })?
                    .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
                    .header(SimpleHeader::RANGE, range_header_value(range))
                    .build(),
            )
            .await
            .map_err(|e| StorageError::Backend(format!("R2 GET request failed: {e}")))?;

        if response.get_status() == Status::NotFound {
            return Ok(empty_blob_stream());
        }
        if response.get_status() != Status::PartialContent {
            return Err(StorageError::Backend(format!(
                "R2 range GET failed with status {}",
                response.get_status()
            )));
        }
        let (_, _, body) = response.into_async_parts();
        Ok(body_to_blob_stream(body))
    }

    /// Async PUT with optional conditional headers (`If-None-Match` / `If-Match`).
    #[allow(clippy::too_many_arguments)]
    async fn put_object_conditional_async(
        &self,
        key: &str,
        data: &[u8],
        content_type: &str,
        if_none_match: Option<&str>,
        if_match: Option<&str>,
    ) -> Result<(), StorageError> {
        let url = self.object_url(key);
        let mut builder = PreparedRequestBuilder::put(&url)
            .map_err(|e| StorageError::Backend(format!("R2 PUT request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .header(SimpleHeader::CONTENT_TYPE, content_type);
        if let Some(value) = if_none_match {
            builder = builder.header(SimpleHeader::IF_NONE_MATCH, value);
        }
        if let Some(value) = if_match {
            builder = builder.header(SimpleHeader::IF_MATCH, value);
        }
        let response = self
            .client
            .send_async(builder.body_bytes(data.to_vec()).build())
            .await
            .map_err(|e| StorageError::Backend(format!("R2 PUT request failed: {e}")))?;

        let status = response.get_status();
        if status == Status::PreconditionFailed {
            return Err(StorageError::PreconditionFailed(key.to_string()));
        }
        let status_code: usize = status.into();
        if status_code >= 400 {
            return Err(StorageError::Backend(format!(
                "R2 PUT failed with status {status_code}"
            )));
        }
        Ok(())
    }

    /// Async streamed PUT — pushes chunks through a bounded pipe so the body
    /// is never buffered whole.
    async fn put_object_stream_async(
        &self,
        key: &str,
        content_type: &str,
        length: Option<u64>,
        chunks: BlobStream,
    ) -> Result<(), StorageError> {
        let (producer, body) = pushable_request_body();
        let url = self.object_url(key);
        let mut builder = PreparedRequestBuilder::put(&url)
            .map_err(|e| StorageError::Backend(format!("R2 PUT request build failed: {e}")))?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .header(SimpleHeader::CONTENT_TYPE, content_type);
        if let Some(len) = length {
            builder = builder.header(SimpleHeader::CONTENT_LENGTH, len.to_string());
        }
        let request = builder.body(body).build();

        let send_fut = self.client.send_async(request);
        let push_fut = async move {
            let mut chunks = chunks;
            while let Some(chunk) = chunks.next().await {
                let chunk = chunk?;
                producer
                    .push(chunk)
                    .await
                    .map_err(|e| StorageError::Backend(format!("R2 PUT body push failed: {e}")))?;
            }
            producer.close();
            Ok::<(), StorageError>(())
        };

        let (push_result, send_result) = futures_lite::future::zip(push_fut, send_fut).await;
        let response = send_result
            .map_err(|e| StorageError::Backend(format!("R2 PUT request failed: {e}")))?;
        let status = response.get_status();
        if status == Status::PreconditionFailed {
            return Err(StorageError::PreconditionFailed(key.to_string()));
        }
        push_result?;
        let status_code: usize = status.into();
        if status_code >= 400 {
            return Err(StorageError::Backend(format!(
                "R2 PUT failed with status {status_code}"
            )));
        }
        Ok(())
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
            "{}/r2/buckets/{}/objects?prefix={}",
            self.api_prefix,
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
                    .map_err(|e| {
                        StorageError::Backend(format!("R2 LIST request build failed: {e}"))
                    })?
                    .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
                    .build(),
            )
            .await
            .map_err(|e| StorageError::Backend(format!("R2 LIST request failed: {e}")))?;

        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!(
                "R2 LIST failed with status {}",
                response.get_status()
            )));
        }
        let (_, _, body) = response.into_async_parts();
        let text = collect_string_async(body)
            .await
            .map_err(|e| StorageError::Backend(format!("R2 body read failed: {e}")))?;
        let parsed = parse_list_response(&text)?;

        let keys = parsed
            .result
            .iter()
            .map(|object| object.key.clone())
            .collect();
        let truncated = parsed
            .result_info
            .as_ref()
            .and_then(|info| info.is_truncated)
            .unwrap_or(false);
        let next = if truncated {
            let cursor = parsed
                .result_info
                .as_ref()
                .and_then(|info| info.cursor.as_deref())
                .filter(|c| !c.is_empty())
                .ok_or_else(|| {
                    StorageError::Serialization("R2 LIST: truncated page has no cursor".to_string())
                })?;
            Some(cursor.to_string())
        } else {
            None
        };
        Ok((keys, next))
    }

    /// A cursor-driven stream of state IDs whose ID starts with `id_prefix`.
    ///
    /// Each page is fetched only when the previous page's IDs have all been
    /// pulled, so the first ID is yielded after one LIST request and the rest
    /// never materialize into a `Vec`. R2 returns object keys in order, so the
    /// stream keeps that order and does not sort.
    fn state_id_stream(&self, id_prefix: &str) -> StateStoreStream<String> {
        let store = self.clone();
        let store_prefix = self.state_prefix();
        let list_prefix = format!("{store_prefix}{}", id_prefix.replace('/', ":"));
        let seed = PageState::Pending {
            pending: VecDeque::new(),
            cursor: None,
            done: false,
        };
        Box::pin(stream::unfold(seed, move |state| {
            let store = store.clone();
            let store_prefix = store_prefix.clone();
            let list_prefix = list_prefix.clone();
            async move {
                match state {
                    PageState::Done => None,
                    PageState::Pending {
                        mut pending,
                        mut cursor,
                        mut done,
                    } => loop {
                        if let Some(id) = pending.pop_front() {
                            return Some((
                                Ok(id),
                                PageState::Pending {
                                    pending,
                                    cursor,
                                    done,
                                },
                            ));
                        }
                        if done {
                            return None;
                        }
                        match store
                            .list_objects_page(&list_prefix, cursor.as_deref())
                            .await
                        {
                            Err(e) => return Some((Err(e), PageState::Done)),
                            Ok((keys, next)) => {
                                pending = keys
                                    .into_iter()
                                    .filter_map(|key| {
                                        key.strip_prefix(&store_prefix)
                                            .and_then(|rest| rest.strip_suffix(".json"))
                                            .map(|id| id.replace(':', "/"))
                                    })
                                    .collect();
                                match next {
                                    Some(c) => cursor = Some(c),
                                    None => done = true,
                                }
                            }
                        }
                    },
                }
            }
        }))
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

    /// Like [`states_for`](Self::states_for), from a stream of IDs.
    fn states_for_stream(&self, ids: StateStoreStream<String>) -> StateStoreStream<ResourceState> {
        let store = self.clone();
        Box::pin(
            ids.then(move |id| {
                let store = store.clone();
                async move {
                    match id {
                        Ok(id) => store.get(&id).await,
                        Err(e) => Err(e),
                    }
                }
            })
            .filter_map(Result::transpose),
        )
    }
}

/// The cursor state for the state-ID paging stream.
enum PageState {
    /// A page of decoded IDs is pending, and `cursor` points at the next page
    /// (or `None` when the last page has been requested).
    Pending {
        pending: VecDeque<String>,
        cursor: Option<String>,
        /// True once the last page has been fetched, so the stream stops
        /// instead of restarting the listing from `cursor == None`.
        done: bool,
    },
    /// The stream ended, or a page failed.
    Done,
}

/// Parse an R2 object-listing response.
///
/// Wrangler's local explorer API (beta) writes `result_info.is_truncated` as
/// the string `"false"`/`"true"`; the published spec and the hosted API use a
/// boolean. Both are accepted: a string is turned into a boolean before the
/// typed parse.
fn parse_list_response(text: &str) -> Result<R2ListObjectsResponse, StorageError> {
    let parse_err =
        |e: serde_json::Error| StorageError::Serialization(format!("R2 LIST parse failed: {e}"));
    let mut value: serde_json::Value = serde_json::from_str(text).map_err(parse_err)?;
    if let Some(flag) = value.pointer_mut("/result_info/is_truncated") {
        if let Some(text) = flag.as_str() {
            *flag = serde_json::Value::Bool(text.eq_ignore_ascii_case("true"));
        }
    }
    serde_json::from_value(value).map_err(parse_err)
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

/// Wrap an [`AsyncSendSafeBody`] as a [`BlobStream`], mapping transport chunk
/// errors to [`StorageError::Backend`]. The body is pulled one chunk at a time;
/// it is never buffered whole.
fn body_to_blob_stream(body: AsyncSendSafeBody) -> BlobStream {
    Box::pin(body.map(|result| {
        result.map_err(|e| StorageError::Backend(format!("R2 body read failed: {e}")))
    }))
}

/// An already-finished [`BlobStream`] with no chunks.
fn empty_blob_stream() -> BlobStream {
    Box::pin(futures_lite::stream::empty())
}

/// The `Range` header value for a half-open `[start, end)` [`BlobRange`]:
/// `bytes={start}-{end-1}`, or `bytes={start}-` when the end is unbounded.
fn range_header_value(range: BlobRange) -> String {
    match range.end {
        Some(end) if end > range.start => format!("bytes={}-{}", range.start, end - 1),
        _ => format!("bytes={}-", range.start),
    }
}

// ===========================================================================
// BlobStore
// ===========================================================================

#[async_trait::async_trait]
impl BlobStore for R2Store {
    async fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        self.put_object_async(&self.blob_object_key(key), data, "application/octet-stream")
            .await
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

    async fn get_blob_stream(&self, key: &str) -> StorageResult<BlobStream> {
        match self
            .get_object_stream_async(&self.blob_object_key(key))
            .await?
        {
            Some(stream) => Ok(stream),
            None => Ok(empty_blob_stream()),
        }
    }

    async fn put_blob_stream(
        &self,
        key: &str,
        length: Option<u64>,
        chunks: BlobStream,
    ) -> StorageResult<()> {
        self.put_object_stream_async(
            &self.blob_object_key(key),
            "application/octet-stream",
            length,
            chunks,
        )
        .await
    }

    async fn blob_etag(&self, key: &str) -> StorageResult<Option<String>> {
        self.head_object_etag_async(&self.blob_object_key(key))
            .await
    }

    async fn get_blob_range(&self, key: &str, range: BlobRange) -> StorageResult<BlobStream> {
        if range.end.is_some_and(|end| end <= range.start) {
            return Ok(empty_blob_stream());
        }
        self.get_object_range_async(&self.blob_object_key(key), range)
            .await
    }

    async fn put_blob_conditionally(
        &self,
        key: &str,
        data: &[u8],
        condition: BlobPutCondition,
    ) -> StorageResult<()> {
        let (if_none_match, if_match) = match condition {
            BlobPutCondition::IfNoneMatch => (Some("*"), None),
            BlobPutCondition::IfMatch(expected) => (None, Some(expected)),
        };
        self.put_object_conditional_async(
            &self.blob_object_key(key),
            data,
            "application/octet-stream",
            if_none_match,
            if_match.as_deref(),
        )
        .await
    }

    async fn list_blobs_after(
        &self,
        prefix: &str,
        start_after: Option<&str>,
        limit: Option<usize>,
    ) -> StorageResult<BlobListStream> {
        // The object-key form of an empty blob key is the directory prefix
        // (`{blob_prefix}/`); the listing prefix is the object-key form of
        // `prefix`. R2 pages are pulled until the cursor ends.
        let strip = self.blob_object_key("");
        let list_prefix = self.blob_object_key(prefix);
        let mut keys: Vec<String> = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let (page, next) = self
                .list_objects_page(&list_prefix, cursor.as_deref())
                .await?;
            keys.extend(page.into_iter().filter_map(|object_key| {
                object_key
                    .strip_prefix(&strip)
                    .map(|rest| rest.replace(':', "/"))
            }));
            match next {
                Some(next_cursor) => cursor = Some(next_cursor),
                None => break,
            }
        }
        keys.retain(|k| start_after.is_none_or(|s| k.as_str() > s));
        keys.sort();
        keys.dedup();
        if let Some(limit) = limit {
            keys.truncate(limit);
        }
        Ok(Box::pin(futures_lite::stream::iter(
            keys.into_iter().map(Ok),
        )))
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
        self.count_by_prefix("").await
    }

    async fn get(&self, resource_id: &str) -> StorageResult<Option<ResourceState>> {
        let Some(bytes) = self
            .get_object_async(&self.state_object_key(resource_id))
            .await?
        else {
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

    async fn set(&self, resource_id: &str, state: &ResourceState) -> StorageResult<()> {
        let state = state.with_id(resource_id);
        let body = serde_json::to_vec_pretty(&state)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put_object_async(&self.state_object_key(&state.id), &body, "application/json")
            .await
    }

    async fn delete(&self, resource_id: &str) -> StorageResult<()> {
        self.delete_object_async(&self.state_object_key(resource_id))
            .await
    }

    async fn list_by_prefix(&self, prefix: &str) -> StorageResult<StateStoreStream<String>> {
        Ok(self.state_id_stream(prefix))
    }

    async fn count_by_prefix(&self, prefix: &str) -> StorageResult<usize> {
        let mut ids = self.state_id_stream(prefix);
        let mut count = 0;
        while let Some(id) = ids.next().await {
            id?;
            count += 1;
        }
        Ok(count)
    }

    async fn all_by_prefix(&self, prefix: &str) -> StorageResult<StateStoreStream<ResourceState>> {
        let ids = self.state_id_stream(prefix);
        Ok(self.states_for_stream(ids))
    }

    // `find_by_*` and `delete_by_prefix` use the trait defaults: R2 can't
    // filter on the server, so they scan `all()` and filter / list then delete.
}
