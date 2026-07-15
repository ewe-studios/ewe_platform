use std::sync::Arc;

use foundation_core::url::Uri;
use foundation_netio::shared::client::body_reader::collect_strings_from_send_safe;
use foundation_netio::shared::client::http_client::HttpClient;
use foundation_netio::shared::client::request::{Extensions, PreparedRequest};
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod};

use crate::store::{
    AsyncVectorStore, VectorEntry, VectorMatch, VectorStore, VectorStoreConfig, VectorStoreError,
};
use crate::metric::DistanceMetric;
use crate::vector::Vector;
use serde::{Deserialize, Serialize};

pub struct TurboPufferVectorStore {
    client: Arc<dyn HttpClient>,
    api_key: String,
    base_url: String,
    config: VectorStoreConfig,
}

impl TurboPufferVectorStore {
    #[must_use]
    pub fn new(client: Arc<dyn HttpClient>, api_key: String, config: VectorStoreConfig) -> Self {
        Self {
            client,
            api_key,
            base_url: String::from("https://api.turbopuffer.com"),
            config,
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
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
        headers.insert(
            SimpleHeader::ACCEPT,
            vec![String::from("application/json")],
        );
        headers
    }

    fn namespace_url(&self, namespace: &str) -> Result<Uri, VectorStoreError> {
        let url = format!("{}/v1/vectors/{}", self.base_url, namespace);
        Uri::parse(&url).map_err(|e| VectorStoreError::Backend(format!("invalid URL: {e}")))
    }

    fn send_request(
        &self,
        method: SimpleMethod,
        url: Uri,
        body: SendSafeBody,
    ) -> Result<String, VectorStoreError> {
        let req = PreparedRequest {
            method,
            url,
            headers: self.auth_headers(),
            body,
            extensions: Extensions::default(),
        };
        let response = self
            .client
            .send(req)
            .map_err(|e| VectorStoreError::Backend(format!("HTTP error: {e}")))?;
        let (status, _headers, body) = response.into_parts();
        let status_code: usize = status.into();
        let body_text = collect_strings_from_send_safe(body)
            .map_err(|e| VectorStoreError::Backend(format!("body read error: {e}")))?;
        if !(200..=299).contains(&status_code) {
            return Err(VectorStoreError::Backend(format!(
                "HTTP {status_code}: {body_text}"
            )));
        }
        Ok(body_text)
    }

    fn distance_metric_name(&self) -> &'static str {
        match self.config.metric {
            DistanceMetric::Cosine => "cosine_distance",
            DistanceMetric::L2 => "euclidean_squared",
            DistanceMetric::Dot => "cosine_distance",
        }
    }
}

#[derive(Serialize)]
struct UpsertRequest {
    ids: Vec<String>,
    vectors: Vec<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attributes: Option<UpsertAttributes>,
}

#[derive(Serialize)]
struct UpsertAttributes {
    metadata: Vec<String>,
}

#[derive(Serialize)]
struct DeleteRequest {
    ids: Vec<String>,
    vectors: Vec<serde_json::Value>,
}

#[derive(Serialize)]
struct QueryRequest {
    vector: Vec<f32>,
    top_k: usize,
    distance_metric: String,
    include_attributes: Vec<String>,
}

#[derive(Deserialize)]
struct QueryResult {
    id: String,
    dist: f32,
    #[serde(default)]
    attributes: Option<QueryAttributes>,
    #[serde(default)]
    vector: Option<Vec<f32>>,
}

#[derive(Deserialize)]
struct QueryAttributes {
    #[serde(default)]
    metadata: Option<String>,
}

impl VectorStore for TurboPufferVectorStore {
    fn insert(&self, namespace: &str, entry: VectorEntry) -> Result<(), VectorStoreError> {
        if entry.vector.dimension() != self.config.dimension {
            return Err(VectorStoreError::DimensionMismatch {
                expected: self.config.dimension,
                got: entry.vector.dimension(),
            });
        }
        if entry.vector.is_zero() {
            return Err(VectorStoreError::ZeroVector);
        }
        let meta_json = serde_json::to_string(&entry.metadata)
            .map_err(|e| VectorStoreError::Backend(format!("serialize metadata: {e}")))?;
        let req = UpsertRequest {
            ids: vec![entry.id],
            vectors: vec![entry.vector.data],
            attributes: Some(UpsertAttributes {
                metadata: vec![meta_json],
            }),
        };
        let body = serde_json::to_string(&req)
            .map_err(|e| VectorStoreError::Backend(format!("serialize: {e}")))?;
        let url = self.namespace_url(namespace)?;
        self.send_request(SimpleMethod::POST, url, SendSafeBody::Text(body))?;
        Ok(())
    }

    fn delete(&self, namespace: &str, id: &str) -> Result<(), VectorStoreError> {
        let req = DeleteRequest {
            ids: vec![id.to_string()],
            vectors: vec![serde_json::Value::String("undef".into())],
        };
        let body = serde_json::to_string(&req)
            .map_err(|e| VectorStoreError::Backend(format!("serialize: {e}")))?;
        let url = self.namespace_url(namespace)?;
        self.send_request(SimpleMethod::POST, url, SendSafeBody::Text(body))?;
        Ok(())
    }

    fn search(
        &self,
        namespace: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<VectorMatch>, VectorStoreError> {
        if query.len() != self.config.dimension {
            return Err(VectorStoreError::DimensionMismatch {
                expected: self.config.dimension,
                got: query.len(),
            });
        }
        let req = QueryRequest {
            vector: query.to_vec(),
            top_k: k,
            distance_metric: self.distance_metric_name().to_string(),
            include_attributes: vec!["metadata".into()],
        };
        let body = serde_json::to_string(&req)
            .map_err(|e| VectorStoreError::Backend(format!("serialize: {e}")))?;
        let url_str = format!("{}/v1/vectors/{}/query", self.base_url, namespace);
        let url = Uri::parse(&url_str)
            .map_err(|e| VectorStoreError::Backend(format!("invalid URL: {e}")))?;
        let resp_text =
            self.send_request(SimpleMethod::POST, url, SendSafeBody::Text(body))?;
        let results: Vec<QueryResult> = serde_json::from_str(&resp_text)
            .map_err(|e| VectorStoreError::Backend(format!("deserialize: {e}")))?;
        Ok(results
            .into_iter()
            .map(|r| VectorMatch {
                id: r.id,
                score: 1.0 - r.dist,
            })
            .collect())
    }

    fn get(&self, namespace: &str, id: &str) -> Result<Option<VectorEntry>, VectorStoreError> {
        let dummy_query = vec![0.0_f32; self.config.dimension];
        let req = QueryRequest {
            vector: dummy_query,
            top_k: 1,
            distance_metric: self.distance_metric_name().to_string(),
            include_attributes: vec!["metadata".into()],
        };
        let body = serde_json::to_string(&req)
            .map_err(|e| VectorStoreError::Backend(format!("serialize: {e}")))?;
        let url_str = format!("{}/v1/vectors/{}/query", self.base_url, namespace);
        let url = Uri::parse(&url_str)
            .map_err(|e| VectorStoreError::Backend(format!("invalid URL: {e}")))?;
        let resp_text =
            self.send_request(SimpleMethod::POST, url, SendSafeBody::Text(body))?;
        let results: Vec<QueryResult> = serde_json::from_str(&resp_text)
            .map_err(|e| VectorStoreError::Backend(format!("deserialize: {e}")))?;
        Ok(results.into_iter().find(|r| r.id == id).map(|r| {
            let metadata = r
                .attributes
                .and_then(|a| a.metadata)
                .and_then(|m| serde_json::from_str(&m).ok())
                .unwrap_or_default();
            VectorEntry {
                id: r.id,
                vector: Vector::new(r.vector.unwrap_or_default()),
                metadata,
            }
        }))
    }

    fn len(&self, _namespace: &str) -> usize {
        0
    }

    fn config(&self) -> &VectorStoreConfig {
        &self.config
    }
}

#[async_trait::async_trait]
impl AsyncVectorStore for TurboPufferVectorStore {
    async fn insert_async(
        &self,
        namespace: &str,
        entry: VectorEntry,
    ) -> Result<(), VectorStoreError> {
        VectorStore::insert(self, namespace, entry)
    }

    async fn delete_async(&self, namespace: &str, id: &str) -> Result<(), VectorStoreError> {
        VectorStore::delete(self, namespace, id)
    }

    async fn search_async(
        &self,
        namespace: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<VectorMatch>, VectorStoreError> {
        VectorStore::search(self, namespace, query, k)
    }

    async fn get_async(
        &self,
        namespace: &str,
        id: &str,
    ) -> Result<Option<VectorEntry>, VectorStoreError> {
        VectorStore::get(self, namespace, id)
    }

    async fn len_async(&self, namespace: &str) -> usize {
        VectorStore::len(self, namespace)
    }

    fn config(&self) -> &VectorStoreConfig {
        &self.config
    }
}
