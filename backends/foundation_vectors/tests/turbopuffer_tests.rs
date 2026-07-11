//! `TurboPuffer` `VectorStore`/`AsyncVectorStore` adapter tests (F30) against a
//! mock `HttpClient` — no network.
//!
//! WHY: `TurboPuffer` is the REQUIRED external vector backend. The adapter is a
//! thin REST translation over F00f's `HttpClient` trait, so a mock client lets us
//! assert the exact request shapes (URL, method, auth, body) and response parsing
//! deterministically, on every platform.
//!
//! WHAT: upsert (insert) request shape + dimension/zero guards, query (search)
//! request + `dist → score` mapping, delete request, and the Bearer auth header.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use foundation_netio::simple_http::client::shared::http_client::{
    BoxedSseFutureStream, BoxedSseIterator, HttpClient,
};
use foundation_netio::simple_http::client::shared::request::PreparedRequest;
use foundation_netio::simple_http::client::shared::HttpExchangeClientTask;
use foundation_netio::simple_http::shared::{
    HttpClientError, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleResponse, Status,
};

use foundation_vectors::store::{
    AsyncVectorStore, VectorEntry, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::vector::Vector;
use foundation_vectors::TurboPufferVectorStore;

/// One captured outgoing request.
#[derive(Clone)]
struct Captured {
    method: String,
    url: String,
    body: String,
    auth: Option<String>,
}

/// Mock `HttpClient`: records every request and replies with a canned body for
/// `/query` paths (200) and an empty 200 otherwise.
struct MockHttpClient {
    calls: Mutex<Vec<Captured>>,
    query_body: String,
}

impl MockHttpClient {
    fn new(query_body: &str) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            query_body: query_body.to_string(),
        })
    }

    fn capture(&self, req: &PreparedRequest) -> bool {
        let body = match &req.body {
            SendSafeBody::Text(s) => s.clone(),
            SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
            _ => String::new(),
        };
        let url = req.url.to_string();
        let is_query = url.ends_with("/query");
        let auth = req
            .headers
            .get(&SimpleHeader::AUTHORIZATION)
            .and_then(|v| v.first().cloned());
        self.calls.lock().unwrap().push(Captured {
            method: req.method.to_string(),
            url,
            body,
            auth,
        });
        is_query
    }

    fn reply(&self, is_query: bool) -> SimpleResponse<SendSafeBody> {
        let body = if is_query { self.query_body.clone() } else { "[]".to_string() };
        SimpleResponse::new(Status::OK, SimpleHeaders::new(), SendSafeBody::Text(body))
    }
}

#[async_trait]
impl HttpClient for MockHttpClient {
    async fn send_async(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        let is_query = self.capture(&req);
        Ok(self.reply(is_query))
    }

    async fn send_sse_async(
        &self,
        _req: PreparedRequest,
    ) -> Result<BoxedSseFutureStream, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }

    fn send(&self, req: PreparedRequest) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        let is_query = self.capture(&req);
        Ok(self.reply(is_query))
    }

    fn send_sse(&self, _req: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }

    fn open_exchange(&self, _req: PreparedRequest) -> HttpExchangeClientTask {
        unimplemented!("open_exchange not implemented for MockHttpClient")
    }
}

fn store(client: Arc<MockHttpClient>) -> TurboPufferVectorStore {
    let config = VectorStoreConfig::new(3, DistanceMetric::Cosine);
    TurboPufferVectorStore::new(client, "test-key".into(), config)
}

fn entry(id: &str, v: Vec<f32>) -> VectorEntry {
    VectorEntry {
        id: id.into(),
        vector: Vector::new(v),
        metadata: VectorMetadata::default(),
    }
}

#[test]
fn insert_posts_upsert_with_auth_and_body() {
    let mock = MockHttpClient::new("[]");
    let store = store(mock.clone());

    store.insert("ns1", entry("a", vec![1.0, 0.0, 0.0])).unwrap();

    let calls = mock.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    let c = &calls[0];
    assert_eq!(c.method, "POST");
    assert!(c.url.ends_with("/v1/vectors/ns1"), "namespace URL: {}", c.url);
    assert_eq!(c.auth.as_deref(), Some("Bearer test-key"));
    // Upsert body carries the id and the vector.
    assert!(c.body.contains("\"a\""));
    assert!(c.body.contains("1.0") || c.body.contains('1'));
}

#[test]
fn insert_rejects_dimension_mismatch_before_http() {
    let mock = MockHttpClient::new("[]");
    let store = store(mock.clone());

    let err = store.insert("ns", entry("a", vec![1.0, 2.0])).unwrap_err();
    assert!(matches!(err, VectorStoreError::DimensionMismatch { expected: 3, got: 2 }));
    assert!(mock.calls.lock().unwrap().is_empty(), "no HTTP on validation failure");
}

#[test]
fn insert_rejects_zero_vector() {
    let mock = MockHttpClient::new("[]");
    let store = store(mock.clone());
    let err = store.insert("ns", entry("a", vec![0.0, 0.0, 0.0])).unwrap_err();
    assert!(matches!(err, VectorStoreError::ZeroVector));
    assert!(mock.calls.lock().unwrap().is_empty());
}

#[test]
fn search_posts_query_and_maps_dist_to_score() {
    // TurboPuffer returns distances; the adapter maps score = 1 - dist.
    let mock = MockHttpClient::new(r#"[{"id":"a","dist":0.1},{"id":"b","dist":0.4}]"#);
    let store = store(mock.clone());

    let matches = store.search("ns1", &[1.0, 0.0, 0.0], 2).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].id, "a");
    assert!((matches[0].score - 0.9).abs() < 1e-6);
    assert!((matches[1].score - 0.6).abs() < 1e-6);

    let calls = mock.calls.lock().unwrap();
    let c = calls.last().unwrap();
    assert!(c.url.ends_with("/v1/vectors/ns1/query"), "query URL: {}", c.url);
    assert!(c.body.contains("\"top_k\":2"));
    assert!(c.body.contains("cosine_distance"));
}

#[test]
fn search_rejects_dimension_mismatch() {
    let mock = MockHttpClient::new("[]");
    let store = store(mock.clone());
    let err = store.search("ns", &[1.0, 0.0], 5).unwrap_err();
    assert!(matches!(err, VectorStoreError::DimensionMismatch { expected: 3, got: 2 }));
    assert!(mock.calls.lock().unwrap().is_empty());
}

#[test]
fn delete_posts_to_namespace() {
    let mock = MockHttpClient::new("[]");
    let store = store(mock.clone());
    store.delete("ns1", "a").unwrap();
    let calls = mock.calls.lock().unwrap();
    let c = calls.last().unwrap();
    assert_eq!(c.method, "POST");
    assert!(c.url.ends_with("/v1/vectors/ns1"));
    assert!(c.body.contains("\"a\""));
}

#[test]
fn async_surface_delegates_to_sync() {
    let mock = MockHttpClient::new(r#"[{"id":"a","dist":0.2}]"#);
    let store = store(mock.clone());
    futures_lite::future::block_on(async {
        store.insert_async("ns", entry("a", vec![1.0, 0.0, 0.0])).await.unwrap();
        let m = store.search_async("ns", &[1.0, 0.0, 0.0], 1).await.unwrap();
        assert_eq!(m.len(), 1);
        assert!((m[0].score - 0.8).abs() < 1e-6);
    });
}
