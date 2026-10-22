//! `TurboPuffer` vector store over the `TurboPuffer` v2 REST API (F30).
//!
//! WHY: `TurboPuffer` is the managed vector database the platform uses when a
//! session needs vectors outside the process. The adapter is a thin REST
//! translation over the platform's [`HttpClient`] seam, so it runs natively and
//! in a Cloudflare Worker with the same code.
//!
//! WHAT: [`TurboPufferVectorStore`] implements the async [`VectorStore`]:
//!
//! | Method | Request |
//! |---|---|
//! | `insert_async` | `POST /v2/namespaces/{ns}` with `upsert_rows` |
//! | `delete_async` | `POST /v2/namespaces/{ns}` with `deletes` and `return_affected_ids` |
//! | `search_async` | `POST /v2/namespaces/{ns}/query` with `rank_by: ["vector", "ANN", q]` |
//! | `get_async` | `POST /v2/namespaces/{ns}/query` with `filters: ["id", "Eq", id]` |
//! | `len_async` | `POST /v2/namespaces/{ns}/query` with `aggregate_by: {"count": ["Count"]}` |
//!
//! HOW: every request goes through [`HttpClient::send_async`], so no method
//! blocks a worker. The status is checked before the body is read. A 2xx body
//! is a small JSON object that the next step needs whole, so it is read up to
//! [`TurboPufferLimits::max_response_body`]. A non-2xx body is read only up to
//! [`TurboPufferLimits::max_error_body`], for the error message. A query on a
//! namespace that does not exist (HTTP 404) reads as an empty namespace.
//!
//! The entry metadata is stored as one string attribute, `metadata` (JSON).
//!
//! `TurboPuffer` has two distance metrics: `cosine_distance` and
//! `euclidean_squared`. A store configured with [`DistanceMetric::Dot`] fails to
//! build with [`VectorStoreError::Unsupported`]. Scores keep the crate's
//! "higher is closer" rule: cosine gives `1 - dist`, L2 gives `-dist` (the
//! negative squared distance, the same as [`DistanceMetric::L2`]).

use std::collections::HashMap;
use std::sync::Arc;

use foundation_core::url::Uri;
use foundation_netio::shared::client::body_reader::AsyncSendSafeBody;
use foundation_netio::shared::client::http_client::HttpClient;
use foundation_netio::shared::client::request::{Extensions, PreparedRequest};
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod};
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

pub struct TurboPufferVectorStore {
    client: Arc<dyn HttpClient>,
    api_key: String,
    base_url: String,
    config: VectorStoreConfig,
    limits: TurboPufferLimits,
}

/// A 2xx body, or the HTTP 404 of a namespace that does not exist.
enum Reply {
    Ok(String),
    NotFound,
}

impl TurboPufferVectorStore {
    /// A store that talks to `https://api.turbopuffer.com`. Use
    /// [`with_base_url`](Self::with_base_url) for a region endpoint
    /// (`https://{region}.turbopuffer.com`).
    ///
    /// # Errors
    /// [`VectorStoreError::Unsupported`] for [`DistanceMetric::Dot`]:
    /// `TurboPuffer` has no dot-product metric.
    pub fn new(
        client: Arc<dyn HttpClient>,
        api_key: String,
        config: VectorStoreConfig,
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
        })
    }

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

    fn auth_headers(&self) -> SimpleHeaders {
        let mut headers = SimpleHeaders::new();
        headers.insert(
            SimpleHeader::AUTHORIZATION,
            vec![format!("Bearer {}", self.api_key)],
        );
        headers.insert(
            SimpleHeader::CONTENT_TYPE,
            vec![String::from("application/json")],
        );
        headers.insert(SimpleHeader::ACCEPT, vec![String::from("application/json")]);
        headers
    }

    /// `/v2/namespaces/{ns}{suffix}`. `TurboPuffer` namespace names use only
    /// ASCII letters, digits, `-`, `_` and `.` (at most 128 bytes), so a
    /// valid name needs no URL encoding.
    fn namespace_url(&self, namespace: &str, suffix: &str) -> Result<Uri, VectorStoreError> {
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
        let url = format!("{}/v2/namespaces/{namespace}{suffix}", self.base_url);
        Uri::parse(&url).map_err(|e| VectorStoreError::Backend(format!("invalid URL: {e}")))
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
    async fn post(&self, url: Uri, body: &serde_json::Value) -> Result<Reply, VectorStoreError> {
        let req = PreparedRequest {
            method: SimpleMethod::POST,
            url,
            headers: self.auth_headers(),
            body: SendSafeBody::Text(body.to_string()),
            extensions: Extensions::default(),
        };
        let response = self
            .client
            .send_async(req)
            .await
            .map_err(|e| VectorStoreError::Backend(format!("HTTP error: {e}")))?;
        let (status, _headers, body) = response.into_parts();
        let status_code: usize = status.into();
        if status_code == 404 {
            return Ok(Reply::NotFound);
        }
        if !(200..=299).contains(&status_code) {
            let (text, _) = read_bounded(body, self.limits.max_error_body).await;
            return Err(VectorStoreError::Backend(format!("HTTP {status_code}: {text}")));
        }
        let (text, truncated) = read_bounded(body, self.limits.max_response_body).await;
        if truncated {
            return Err(VectorStoreError::Backend(format!(
                "response body is larger than max_response_body ({} bytes)",
                self.limits.max_response_body
            )));
        }
        Ok(Reply::Ok(text))
    }

    /// A query on `namespace`; `None` when the namespace does not exist.
    async fn query(
        &self,
        namespace: &str,
        body: &serde_json::Value,
    ) -> Result<Option<QueryResponse>, VectorStoreError> {
        let url = self.namespace_url(namespace, "/query")?;
        match self.post(url, body).await? {
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
        match self.post(url, body).await? {
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
async fn read_bounded(body: SendSafeBody, limit: usize) -> (String, bool) {
    let mut body = AsyncSendSafeBody::from(body);
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
    async fn insert_async(
        &self,
        namespace: &str,
        entry: VectorEntry,
    ) -> Result<(), VectorStoreError> {
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

    async fn delete_async(&self, namespace: &str, id: &str) -> Result<(), VectorStoreError> {
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

    async fn search_async(
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
                let dist = r.dist.ok_or_else(|| {
                    VectorStoreError::Backend("query row has no $dist".into())
                })?;
                Ok(VectorMatch {
                    id: id_text(&r.id),
                    score: self.score(dist),
                })
            })
            .collect()
    }

    async fn get_async(
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

    async fn len_async(&self, namespace: &str) -> Result<usize, VectorStoreError> {
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
