//! `TurboPuffer` vector store over the `TurboPuffer` v2 REST API (F30).
//!
//! WHY: `TurboPuffer` is the managed vector database the platform uses when a
//! session needs vectors outside the process. The adapter is a thin REST
//! translation over the platform's network client, [`DynNetClient`]. That
//! client is the native HTTP client on native targets and the fetch client on
//! wasm32 (a browser or a Cloudflare Worker), so the same code runs on both.
//! [`TurboPufferVectorStore::new`] builds the default client with
//! [`HttpClientBuilder`]; [`TurboPufferVectorStore::with_client`] takes a
//! client that the caller built (a shared client, or a test client).
//!
//! WHAT: [`TurboPufferVectorStore`] implements the async [`VectorStore`]:
//!
//! | Method | Request |
//! |---|---|
//! | `insert` | `POST /v2/namespaces/{ns}` with `upsert_rows` |
//! | `delete` | `POST /v2/namespaces/{ns}` with `deletes` and `return_affected_ids` |
//! | `search` | `POST /v2/namespaces/{ns}/query` with `rank_by: ["vector", "ANN", q]` |
//! | `get` | `POST /v2/namespaces/{ns}/query` with `filters: ["id", "Eq", id]` |
//! | `len` | `POST /v2/namespaces/{ns}/query` with `aggregate_by: {"count": ["Count"]}` |
//!
//! HOW: every request is built with [`PreparedRequestBuilder`] and goes
//! through `HttpClient::send_async` on the [`DynNetClient`], so no method
//! blocks a worker. The status is checked before the body is read. A 2xx body
//! is a small JSON object that the next step needs whole, so it is read up to
//! [`TurboPufferLimits::max_response_body`]. A non-2xx body is read only up to
//! [`TurboPufferLimits::max_error_body`], for the error message. A query on a
//! namespace that does not exist (HTTP 404) reads as an empty namespace. A
//! rate limit (HTTP 429) or a brief outage (HTTP 503) is retried with
//! exponential backoff and jitter, honouring `Retry-After`, up to the counts
//! and waits in [`TurboPufferConfig`].
//!
//! The entry metadata is stored as one string attribute, `metadata` (JSON).
//!
//! `TurboPuffer` has two distance metrics: `cosine_distance` and
//! `euclidean_squared`. A store configured with [`DistanceMetric::Dot`] fails to
//! build with [`VectorStoreError::Unsupported`]. Scores keep the crate's
//! "higher is closer" rule: cosine gives `1 - dist`, L2 gives `-dist` (the
//! negative squared distance, the same as [`DistanceMetric::L2`]).

use std::collections::HashMap;
use std::time::Duration;

use foundation_core::valtron::Ctx;
use foundation_netio::shared::client::body_reader::AsyncSendSafeBody;
use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};
use futures_lite::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::metric::DistanceMetric;
use crate::store::{
    VectorEntry, VectorMatch, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use crate::vector::Vector;

/// The attribute that holds an entry's metadata (JSON text).
const METADATA_ATTR: &str = "metadata";

/// Caps on how much of a `TurboPuffer` response body is held in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurboPufferLimits {
    /// The most of a 2xx response body that is read. A larger body is an
    /// error, not a truncated answer.
    ///
    /// Default 16 MiB. Neither nginx nor Go `net/http` caps a response body by
    /// default, so this is sized from the payloads: a search row without its
    /// vector is about 100 bytes (10 000 rows is about 1 MiB), and a `get` row
    /// with a 16 384-dimension vector as JSON is about 200 KiB.
    pub max_response_body: usize,
    /// The most of a non-2xx response body that is read for its error message;
    /// the rest is never read.
    ///
    /// Default 64 KiB, the same as `foundation_deployment`'s `BodyLimits`. It
    /// is the order of what nginx holds of a proxied response in memory by
    /// default (`proxy_buffers 8 4k|8k`, 32 to 64 KiB).
    pub max_error_body: usize,
}

impl Default for TurboPufferLimits {
    fn default() -> Self {
        Self {
            max_response_body: 16 * 1024 * 1024,
            max_error_body: 64 * 1024,
        }
    }
}

impl TurboPufferLimits {
    /// Read at most `bytes` of a 2xx body.
    #[must_use]
    pub fn with_max_response_body(mut self, bytes: usize) -> Self {
        self.max_response_body = bytes;
        self
    }

    /// Read at most `bytes` of an error body.
    #[must_use]
    pub fn with_max_error_body(mut self, bytes: usize) -> Self {
        self.max_error_body = bytes;
        self
    }
}

/// Retry behaviour for `TurboPuffer` rate limits (HTTP 429) and brief outages
/// (HTTP 503).
///
/// Rule 8: the retry count and waits are configuration, not constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurboPufferConfig {
    /// The most attempts for one request: the first send plus its retries.
    ///
    /// Default 5. The AWS SDK standard retry mode uses 3 attempts; this store
    /// allows 5 because one bulk upsert can cross `TurboPuffer`'s per-namespace
    /// rate limit, and a couple of short waits cost less than a failed upload.
    pub retry_attempts: u32,
    /// The wait before the first retry. Each later retry doubles this wait.
    ///
    /// Default 200 ms, the AWS SDK standard retry mode's base delay.
    pub retry_base_delay: Duration,
    /// The most a computed retry wait can be. A `Retry-After` header is honoured
    /// as sent and is not capped.
    ///
    /// Default 10 s, half the AWS SDK standard retry mode's 20 s cap, enough for
    /// a short namespace burst without a long tail.
    pub retry_max_delay: Duration,
}

impl Default for TurboPufferConfig {
    fn default() -> Self {
        Self {
            retry_attempts: 5,
            retry_base_delay: Duration::from_millis(200),
            retry_max_delay: Duration::from_secs(10),
        }
    }
}

impl TurboPufferConfig {
    /// The most attempts for one request (at least 1).
    #[must_use]
    pub fn with_retry_attempts(mut self, attempts: u32) -> Self {
        self.retry_attempts = attempts;
        self
    }

    /// The wait before the first retry.
    #[must_use]
    pub fn with_retry_base_delay(mut self, delay: Duration) -> Self {
        self.retry_base_delay = delay;
        self
    }

    /// The most a computed retry wait can be.
    #[must_use]
    pub fn with_retry_max_delay(mut self, delay: Duration) -> Self {
        self.retry_max_delay = delay;
        self
    }
}

/// A [`VectorStore`] over one `TurboPuffer` account. See the module
/// documentation for the requests that each method sends.
pub struct TurboPufferVectorStore {
    client: DynNetClient,
    api_key: String,
    base_url: String,
    config: VectorStoreConfig,
    limits: TurboPufferLimits,
    retry: TurboPufferConfig,
    /// The context whose clock times the retry waits.
    ctx: Ctx,
}

/// A 2xx body, or the HTTP 404 of a namespace that does not exist.
enum Reply {
    Ok(String),
    NotFound,
}

impl TurboPufferVectorStore {
    /// A store that talks to `https://api.turbopuffer.com` through the
    /// platform's default network client ([`HttpClientBuilder`]: the native
    /// client on native targets, the fetch client on wasm32). Use
    /// [`with_base_url`](Self::with_base_url) for a region endpoint
    /// (`https://{region}.turbopuffer.com`).
    ///
    /// # Errors
    /// [`VectorStoreError::Unsupported`] for [`DistanceMetric::Dot`]:
    /// `TurboPuffer` has no dot-product metric.
    pub fn new(
        api_key: String,
        config: VectorStoreConfig,
        ctx: Ctx,
    ) -> Result<Self, VectorStoreError> {
        Self::with_client(HttpClientBuilder::new().build(), api_key, config, ctx)
    }

    /// A store that sends its requests through `client`: a client that the
    /// caller shares with other code, or a test client. Otherwise the same as
    /// [`new`](Self::new).
    ///
    /// # Errors
    /// [`VectorStoreError::Unsupported`] for [`DistanceMetric::Dot`]:
    /// `TurboPuffer` has no dot-product metric.
    pub fn with_client(
        client: DynNetClient,
        api_key: String,
        config: VectorStoreConfig,
        ctx: Ctx,
    ) -> Result<Self, VectorStoreError> {
        if config.metric == DistanceMetric::Dot {
            return Err(VectorStoreError::Unsupported(
                "TurboPuffer has no dot-product distance; use Cosine or L2".into(),
            ));
        }
        Ok(Self {
            client,
            api_key,
            base_url: String::from("https://api.turbopuffer.com"),
            config,
            limits: TurboPufferLimits::default(),
            retry: TurboPufferConfig::default(),
            ctx,
        })
    }

    /// Send the requests to `base_url` (for example a region endpoint, or a
    /// mock server in tests) in place of `https://api.turbopuffer.com`.
    #[must_use]
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    /// Replace the response body limits.
    #[must_use]
    pub fn with_limits(mut self, limits: TurboPufferLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Replace the retry behaviour.
    #[must_use]
    pub fn with_retry_config(mut self, retry: TurboPufferConfig) -> Self {
        self.retry = retry;
        self
    }

    /// `/v2/namespaces/{ns}{suffix}`. `TurboPuffer` namespace names use only
    /// ASCII letters, digits, `-`, `_` and `.` (at most 128 bytes), so a
    /// valid name needs no URL encoding.
    fn namespace_url(&self, namespace: &str, suffix: &str) -> Result<String, VectorStoreError> {
        let valid = !namespace.is_empty()
            && namespace.len() <= 128
            && namespace
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
        if !valid {
            return Err(VectorStoreError::Unsupported(format!(
                "namespace {namespace:?}: TurboPuffer namespaces use only ASCII letters, \
                 digits, '-', '_' and '.', 1 to 128 bytes"
            )));
        }
        Ok(format!(
            "{}/v2/namespaces/{namespace}{suffix}",
            self.base_url
        ))
    }

    fn distance_metric_name(&self) -> &'static str {
        match self.config.metric {
            DistanceMetric::L2 => "euclidean_squared",
            // `new` refuses Dot, so only Cosine reaches here.
            DistanceMetric::Cosine | DistanceMetric::Dot => "cosine_distance",
        }
    }

    /// Map a `TurboPuffer` distance to the crate's score (higher is closer).
    fn score(&self, dist: f32) -> f32 {
        match self.config.metric {
            DistanceMetric::L2 => -dist,
            DistanceMetric::Cosine | DistanceMetric::Dot => 1.0 - dist,
        }
    }

    /// POST `body` as JSON. The status is checked first; an error body is
    /// read only up to `max_error_body`.
    ///
    /// HTTP 429 and 503 are retried with exponential backoff and jitter, up to
    /// [`TurboPufferConfig::retry_attempts`]. A `Retry-After` header is honoured
    /// in place of the computed wait. The wait uses the valtron timer, never a
    /// thread sleep.
    ///
    /// The returned future is not `Send` on wasm32 (the fetch client's future is
    /// single-threaded). Callers that must return a `Send` future wrap it in
    /// [`foundation_compact::SendWrapper`].
    async fn post(&self, url: &str, body: &serde_json::Value) -> Result<Reply, VectorStoreError> {
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            let req = PreparedRequestBuilder::post(url)
                .map_err(|e| VectorStoreError::Backend(format!("invalid URL {url}: {e}")))?
                .bearer_token(&self.api_key)
                .header(SimpleHeader::ACCEPT, "application/json")
                .body_json(body)
                .map_err(|e| VectorStoreError::Backend(format!("serialize request: {e}")))?
                .build();
            let response = self
                .client
                .send_async(req)
                .await
                .map_err(|e| VectorStoreError::Backend(format!("HTTP error: {e}")))?;
            let (status, headers, body) = response.into_async_parts();
            let status_code: usize = status.into();
            if status_code == 404 {
                return Ok(Reply::NotFound);
            }
            if (200..=299).contains(&status_code) {
                let (text, truncated) = read_bounded(body, self.limits.max_response_body).await;
                if truncated {
                    return Err(VectorStoreError::Backend(format!(
                        "response body is larger than max_response_body ({} bytes)",
                        self.limits.max_response_body
                    )));
                }
                return Ok(Reply::Ok(text));
            }
            let retryable = status_code == 429 || status_code == 503;
            if !retryable || attempt >= self.retry.retry_attempts {
                let (text, _) = read_bounded(body, self.limits.max_error_body).await;
                return Err(VectorStoreError::Backend(format!(
                    "HTTP {status_code}: {text}"
                )));
            }
            // The error body is not needed for a retry, so it is dropped unread.
            // The next request is fresh; no message is kept from this one.
            drop(body);
            let delay = self.retry_delay(&headers, attempt);
            foundation_core::valtron::sleep_async(delay, &self.ctx).await;
        }
    }

    /// The wait before the attempt after `attempt` failed with a retryable
    /// status: `Retry-After` seconds when the header is present, else
    /// exponential backoff with full jitter.
    fn retry_delay(&self, headers: &SimpleHeaders, attempt: u32) -> Duration {
        if let Some(seconds) = retry_after_seconds(headers) {
            return Duration::from_secs(seconds);
        }
        let factor = 2u32.saturating_pow(attempt.saturating_sub(1));
        let backoff = self
            .retry
            .retry_base_delay
            .saturating_mul(factor)
            .min(self.retry.retry_max_delay);
        jitter(backoff)
    }

    /// A query on `namespace`; `None` when the namespace does not exist.
    async fn query(
        &self,
        namespace: &str,
        body: &serde_json::Value,
    ) -> Result<Option<QueryResponse>, VectorStoreError> {
        let url = self.namespace_url(namespace, "/query")?;
        match foundation_compact::SendWrapper::new(self.post(&url, body)).await? {
            Reply::NotFound => Ok(None),
            Reply::Ok(text) => serde_json::from_str(&text)
                .map(Some)
                .map_err(|e| VectorStoreError::Backend(format!("deserialize query: {e}"))),
        }
    }

    /// A write on `namespace`.
    async fn write(
        &self,
        namespace: &str,
        body: &serde_json::Value,
    ) -> Result<Option<WriteResponse>, VectorStoreError> {
        let url = self.namespace_url(namespace, "")?;
        match foundation_compact::SendWrapper::new(self.post(&url, body)).await? {
            Reply::NotFound => Ok(None),
            Reply::Ok(text) => serde_json::from_str(&text)
                .map(Some)
                .map_err(|e| VectorStoreError::Backend(format!("deserialize write: {e}"))),
        }
    }

    fn validate(&self, dim: usize, zero: bool) -> Result<(), VectorStoreError> {
        if dim != self.config.dimension {
            return Err(VectorStoreError::DimensionMismatch {
                expected: self.config.dimension,
                got: dim,
            });
        }
        if zero {
            return Err(VectorStoreError::ZeroVector);
        }
        Ok(())
    }
}

/// Read at most `limit` bytes of `body` as (lossy) UTF-8. The flag is true
/// when the body had more bytes than `limit`; the rest is never read.
async fn read_bounded(mut body: AsyncSendSafeBody, limit: usize) -> (String, bool) {
    let mut buf = Vec::new();
    let mut truncated = false;
    loop {
        match body.next().await {
            Some(Ok(chunk)) => {
                let room = limit - buf.len();
                if chunk.len() > room {
                    buf.extend_from_slice(&chunk[..room]);
                    truncated = true;
                    break;
                }
                buf.extend_from_slice(&chunk);
            }
            Some(Err(e)) => {
                // A body that fails part-way still says what arrived before.
                buf.extend_from_slice(format!(" (body read error: {e})").as_bytes());
                break;
            }
            None => break,
        }
    }
    (String::from_utf8_lossy(&buf).into_owned(), truncated)
}

/// The `Retry-After` header as whole seconds, when it is present and parses as
/// a decimal integer. `TurboPuffer` sends delta-seconds, not an HTTP date; a
/// non-integer value reads as absent, and the caller falls back to backoff.
fn retry_after_seconds(headers: &SimpleHeaders) -> Option<u64> {
    headers
        .get(&SimpleHeader::RETRY_AFTER)?
        .first()?
        .parse::<u64>()
        .ok()
}

/// A random wait in `[0, capped]` (full jitter, the AWS SDK recommendation).
/// The draw uses [`foundation_compact::entropy`]'s platform source (rule 11),
/// not a global RNG. When the source fails, the full capped wait is used: still
/// correct, only less spread.
fn jitter(capped: Duration) -> Duration {
    let nanos = capped.as_nanos();
    let Ok(draw) = foundation_compact::entropy::u32() else {
        return capped;
    };
    let scaled = (u128::from(draw) * nanos) >> 32;
    Duration::from_nanos(u64::try_from(scaled).unwrap_or(u64::MAX))
}

#[derive(Deserialize)]
struct QueryResponse {
    #[serde(default)]
    rows: Option<Vec<Row>>,
    #[serde(default)]
    aggregations: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize)]
struct Row {
    id: serde_json::Value,
    #[serde(default, rename = "$dist")]
    dist: Option<f32>,
    #[serde(default)]
    vector: Option<Vec<f32>>,
    #[serde(default)]
    metadata: Option<String>,
}

#[derive(Deserialize)]
struct WriteResponse {
    #[serde(default)]
    deleted_ids: Option<Vec<serde_json::Value>>,
}

/// A row id as text. Ids written by this store are strings; `TurboPuffer`
/// also allows unsigned integer and UUID ids, which read as their text form.
fn id_text(id: &serde_json::Value) -> String {
    match id {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[derive(Serialize)]
struct UpsertRow<'a> {
    id: &'a str,
    vector: &'a [f32],
    metadata: String,
}

#[async_trait::async_trait]
impl VectorStore for TurboPufferVectorStore {
    async fn insert(&self, namespace: &str, entry: VectorEntry) -> Result<(), VectorStoreError> {
        self.validate(entry.vector.dimension(), entry.vector.is_zero())?;
        let metadata = serde_json::to_string(&entry.metadata)
            .map_err(|e| VectorStoreError::Backend(format!("serialize metadata: {e}")))?;
        let body = json!({
            "upsert_rows": [UpsertRow { id: &entry.id, vector: &entry.vector.data, metadata }],
            "distance_metric": self.distance_metric_name(),
        });
        match self.write(namespace, &body).await? {
            Some(_) => Ok(()),
            None => Err(VectorStoreError::Backend(format!(
                "HTTP 404 on upsert to namespace {namespace}"
            ))),
        }
    }

    async fn delete(&self, namespace: &str, id: &str) -> Result<(), VectorStoreError> {
        let body = json!({ "deletes": [id], "return_affected_ids": true });
        let deleted = self
            .write(namespace, &body)
            .await?
            .and_then(|w| w.deleted_ids)
            .is_some_and(|ids| ids.iter().any(|d| id_text(d) == id));
        if deleted {
            Ok(())
        } else {
            Err(VectorStoreError::NotFound { id: id.to_string() })
        }
    }

    async fn search(
        &self,
        namespace: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<VectorMatch>, VectorStoreError> {
        self.validate(query.len(), false)?;
        if k == 0 {
            return Ok(Vec::new());
        }
        let body = json!({
            "rank_by": ["vector", "ANN", query],
            "top_k": k,
            "distance_metric": self.distance_metric_name(),
        });
        let Some(resp) = self.query(namespace, &body).await? else {
            return Ok(Vec::new());
        };
        resp.rows
            .unwrap_or_default()
            .into_iter()
            .map(|r| {
                let dist = r
                    .dist
                    .ok_or_else(|| VectorStoreError::Backend("query row has no $dist".into()))?;
                Ok(VectorMatch {
                    id: id_text(&r.id),
                    score: self.score(dist),
                })
            })
            .collect()
    }

    async fn get(
        &self,
        namespace: &str,
        id: &str,
    ) -> Result<Option<VectorEntry>, VectorStoreError> {
        let body = json!({
            "rank_by": ["id", "asc"],
            "filters": ["id", "Eq", id],
            "top_k": 1,
            "include_attributes": ["vector", METADATA_ATTR],
        });
        let Some(resp) = self.query(namespace, &body).await? else {
            return Ok(None);
        };
        let Some(row) = resp.rows.unwrap_or_default().into_iter().next() else {
            return Ok(None);
        };
        let vector = row
            .vector
            .ok_or_else(|| VectorStoreError::Backend("row has no vector".into()))?;
        let metadata: VectorMetadata = match row.metadata {
            Some(m) => serde_json::from_str(&m)
                .map_err(|e| VectorStoreError::Backend(format!("decode metadata: {e}")))?,
            None => VectorMetadata::default(),
        };
        Ok(Some(VectorEntry {
            id: id_text(&row.id),
            vector: Vector::new(vector),
            metadata,
        }))
    }

    async fn len(&self, namespace: &str) -> Result<usize, VectorStoreError> {
        let body = json!({ "aggregate_by": { "count": ["Count"] } });
        let Some(resp) = self.query(namespace, &body).await? else {
            return Ok(0);
        };
        let count = resp
            .aggregations
            .and_then(|a| a.get("count").and_then(serde_json::Value::as_u64))
            .ok_or_else(|| VectorStoreError::Backend("query has no count aggregation".into()))?;
        usize::try_from(count)
            .map_err(|e| VectorStoreError::Backend(format!("count out of range: {e}")))
    }

    fn config(&self) -> &VectorStoreConfig {
        &self.config
    }
}
