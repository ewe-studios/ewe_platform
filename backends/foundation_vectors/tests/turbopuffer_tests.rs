//! `TurboPufferVectorStore` tests (F30) against an in-process mock of the
//! `TurboPuffer` v2 REST API. No network leaves the machine.
//!
//! The mock is a small `TurboPuffer`: it keeps rows per namespace, and it
//! answers writes (`upsert_rows`, `deletes`), vector queries (`rank_by` ANN),
//! id lookups (`filters: ["id", "Eq", id]`) and counts (`aggregate_by`). A
//! query on a namespace that was never written is HTTP 404, as on the real
//! service. The store talks to it through the platform's default network
//! client (`HttpClientBuilder`), given to
//! [`TurboPufferVectorStore::with_client`] as a `DynNetClient`, so the tests
//! cover the request shapes, the status handling and the body limits.
//!
//! The shared contract runs against it, plus checks of the request shapes,
//! the error paths and the body limits.

// The mock server is test scaffolding: it favours plain code over these lints.
#![allow(
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    clippy::type_complexity
)]

#[macro_use]
mod contract;

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use foundation_core::valtron::valtron_test;
use foundation_netio::{DynNetClient, HttpClientBuilder};
use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::store::{
    VectorEntry, VectorMetadata, VectorStore, VectorStoreConfig, VectorStoreError,
};
use foundation_vectors::vector::Vector;
use foundation_vectors::{TurboPufferLimits, TurboPufferVectorStore};
use serde_json::{json, Value};

/// One request the mock received.
#[derive(Clone, Debug)]
struct Seen {
    method: String,
    path: String,
    auth: Option<String>,
    body: Value,
}

/// A stored row: vector, metadata attribute.
type Rows = BTreeMap<String, (Vec<f32>, Option<String>)>;

#[derive(Default)]
struct MockState {
    namespaces: HashMap<String, (String, Rows)>,
    seen: Vec<Seen>,
    /// When set, every request gets this status and body.
    fail_with: Option<(u16, String)>,
}

type Shared = Arc<Mutex<MockState>>;

struct Mock {
    state: Shared,
    addr: SocketAddr,
}

impl Mock {
    fn seen(&self) -> Vec<Seen> {
        self.state.lock().unwrap().seen.clone()
    }
}

fn respond(stream: &mut TcpStream, status: u16, body: &str) {
    let head = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body.as_bytes()).unwrap();
    stream.flush().unwrap();
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    1.0 - dot / (na * nb)
}

fn euclidean_squared(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
}

fn floats(v: &Value) -> Vec<f32> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap() as f32)
        .collect()
}

/// Answer one v2 request against the mock state: `(status, body)`.
fn answer(state: &mut MockState, path: &str, body: &Value) -> (u16, String) {
    let rest = path.strip_prefix("/v2/namespaces/").unwrap();
    let (ns, is_query) = match rest.strip_suffix("/query") {
        Some(ns) => (ns.to_string(), true),
        None => (rest.to_string(), false),
    };

    if !is_query {
        let metric = body["distance_metric"].as_str().map(str::to_string);
        let mut deleted = Vec::new();
        let mut affected = 0;
        if let Some(rows) = body["upsert_rows"].as_array() {
            let entry = state
                .namespaces
                .entry(ns.clone())
                .or_insert_with(|| (metric.clone().unwrap(), Rows::new()));
            for row in rows {
                let id = row["id"].as_str().unwrap().to_string();
                let meta = row["metadata"].as_str().map(str::to_string);
                entry.1.insert(id, (floats(&row["vector"]), meta));
                affected += 1;
            }
        }
        if let Some(ids) = body["deletes"].as_array() {
            if let Some((_, rows)) = state.namespaces.get_mut(&ns) {
                for id in ids {
                    let id = id.as_str().unwrap();
                    if rows.remove(id).is_some() {
                        deleted.push(json!(id));
                    }
                }
            }
            affected += ids.len();
        }
        let mut reply = json!({
            "status": "OK",
            "message": "ok",
            "rows_affected": affected,
            "billing": {},
        });
        if body["return_affected_ids"] == json!(true) && !deleted.is_empty() {
            reply["deleted_ids"] = Value::Array(deleted);
        }
        return (200, reply.to_string());
    }

    let Some((metric, rows)) = state.namespaces.get(&ns) else {
        return (404, json!({"status": "error", "error": "namespace not found"}).to_string());
    };

    if body.get("aggregate_by").is_some() {
        let reply = json!({ "aggregations": { "count": rows.len() }, "billing": {}, "performance": {} });
        return (200, reply.to_string());
    }

    let rank_by = body["rank_by"].as_array().unwrap();
    let top_k = body["top_k"].as_u64().unwrap() as usize;
    let include: Vec<&str> = body["include_attributes"]
        .as_array()
        .map(|a| a.iter().map(|v| v.as_str().unwrap()).collect())
        .unwrap_or_default();
    let render = |id: &str, vector: &[f32], meta: &Option<String>, dist: Option<f32>| {
        let mut row = json!({ "id": id });
        if let Some(d) = dist {
            row["$dist"] = json!(d);
        }
        if include.contains(&"vector") {
            row["vector"] = json!(vector);
        }
        if include.contains(&"metadata") {
            if let Some(m) = meta {
                row["metadata"] = json!(m);
            }
        }
        row
    };

    let out: Vec<Value> = if rank_by[1] == json!("ANN") {
        let q = floats(&rank_by[2]);
        let mut scored: Vec<(f32, &String, &(Vec<f32>, Option<String>))> = rows
            .iter()
            .map(|(id, r)| {
                let d = if metric == "euclidean_squared" {
                    euclidean_squared(&q, &r.0)
                } else {
                    cosine_distance(&q, &r.0)
                };
                (d, id, r)
            })
            .collect();
        scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        scored
            .into_iter()
            .take(top_k)
            .map(|(d, id, r)| render(id, &r.0, &r.1, Some(d)))
            .collect()
    } else {
        // rank_by ["id", "asc"] with an optional ["id", "Eq", x] filter.
        let want = body["filters"]
            .as_array()
            .filter(|f| f[0] == json!("id") && f[1] == json!("Eq"))
            .map(|f| f[2].as_str().unwrap().to_string());
        rows.iter()
            .filter(|(id, _)| want.as_ref().is_none_or(|w| w == *id))
            .take(top_k)
            .map(|(id, r)| render(id, &r.0, &r.1, None))
            .collect()
    };
    let reply = json!({ "rows": out, "billing": {}, "performance": {} });
    (200, reply.to_string())
}

fn handle(mut stream: TcpStream, state: &Shared) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap() == 0 {
        return;
    }
    let mut content_length = 0usize;
    let mut expects_continue = false;
    let mut auth = None;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let value = value.trim();
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.parse().unwrap();
            }
            if name.eq_ignore_ascii_case("authorization") {
                auth = Some(value.to_string());
            }
            if name.eq_ignore_ascii_case("expect") && value.eq_ignore_ascii_case("100-continue") {
                expects_continue = true;
            }
        }
    }
    // Answer `Expect: 100-continue` like a real server, or the client waits
    // out its continue timeout before it sends the body.
    if expects_continue {
        stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").unwrap();
    }
    let mut raw = vec![0; content_length];
    reader.read_exact(&mut raw).unwrap();
    let body: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap().to_string();
    let path = parts.next().unwrap().to_string();

    let (status, reply) = {
        let mut state = state.lock().unwrap();
        state.seen.push(Seen {
            method,
            path: path.clone(),
            auth,
            body: body.clone(),
        });
        match state.fail_with.clone() {
            Some(fail) => fail,
            None => answer(&mut state, &path, &body),
        }
    };
    respond(&mut stream, status, &reply);
}

fn start_mock() -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let state: Shared = Arc::default();
    let served = Arc::clone(&state);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            let served = Arc::clone(&served);
            thread::spawn(move || handle(stream, &served));
        }
    });
    Mock { state, addr }
}

fn store_for(mock: &Mock, metric: DistanceMetric) -> TurboPufferVectorStore {
    let client: DynNetClient = HttpClientBuilder::new().build();
    TurboPufferVectorStore::with_client(client, "test-key".into(), VectorStoreConfig::new(3, metric))
        .unwrap()
        .with_base_url(format!("http://{}", mock.addr))
}

async fn make_store() -> (Mock, TurboPufferVectorStore) {
    let mock = start_mock();
    let store = store_for(&mock, DistanceMetric::Cosine);
    (mock, store)
}

vector_store_contract!(make_store);

fn entry(id: &str, v: [f32; 3]) -> VectorEntry {
    VectorEntry {
        id: id.into(),
        vector: Vector::new(v.to_vec()),
        metadata: VectorMetadata::default(),
    }
}

#[valtron_test]
async fn insert_sends_v2_upsert_with_auth_and_metric() {
    let (mock, store) = make_store().await;
    store.insert_async("ns1", entry("a", [1.0, 0.0, 0.0])).await.unwrap();

    let seen = mock.seen();
    assert_eq!(seen.len(), 1);
    let s = &seen[0];
    assert_eq!(s.method, "POST");
    assert_eq!(s.path, "/v2/namespaces/ns1");
    assert_eq!(s.auth.as_deref(), Some("Bearer test-key"));
    assert_eq!(s.body["distance_metric"], json!("cosine_distance"));
    assert_eq!(s.body["upsert_rows"][0]["id"], json!("a"));
    assert_eq!(s.body["upsert_rows"][0]["vector"], json!([1.0, 0.0, 0.0]));
    assert_eq!(s.body["upsert_rows"][0]["metadata"], json!("{}"));
}

#[valtron_test]
async fn validation_errors_send_nothing() {
    let (mock, store) = make_store().await;
    assert!(store.insert_async("ns", entry("a", [0.0, 0.0, 0.0])).await.is_err());
    assert!(store.search_async("ns", &[1.0], 1).await.is_err());
    assert!(matches!(
        store.insert_async("bad/ns", entry("a", [1.0, 0.0, 0.0])).await.unwrap_err(),
        VectorStoreError::Unsupported(_)
    ));
    assert!(mock.seen().is_empty(), "no HTTP on a validation failure");
}

#[valtron_test]
async fn search_get_and_len_use_v2_queries() {
    let (mock, store) = make_store().await;
    store.insert_async("ns", entry("a", [1.0, 0.0, 0.0])).await.unwrap();
    store.search_async("ns", &[1.0, 0.0, 0.0], 2).await.unwrap();
    store.get_async("ns", "a").await.unwrap();
    store.len_async("ns").await.unwrap();

    let seen = mock.seen();
    let (search, get, len) = (&seen[1], &seen[2], &seen[3]);
    assert_eq!(search.path, "/v2/namespaces/ns/query");
    assert_eq!(search.body["rank_by"], json!(["vector", "ANN", [1.0, 0.0, 0.0]]));
    assert_eq!(search.body["top_k"], json!(2));
    assert_eq!(get.body["filters"], json!(["id", "Eq", "a"]));
    assert_eq!(get.body["include_attributes"], json!(["vector", "metadata"]));
    assert_eq!(len.body["aggregate_by"], json!({"count": ["Count"]}));
}

#[valtron_test]
async fn l2_uses_euclidean_squared_and_negative_scores() {
    let mock = start_mock();
    let store = store_for(&mock, DistanceMetric::L2);
    store.insert_async("ns", entry("near", [1.0, 0.0, 0.0])).await.unwrap();
    store.insert_async("ns", entry("far", [3.0, 0.0, 0.0])).await.unwrap();
    let m = store.search_async("ns", &[0.0, 0.0, 1.0], 2).await.unwrap();
    assert_eq!(m[0].id, "near");
    assert!((m[0].score - -2.0).abs() < 1e-5, "score {}", m[0].score);
    assert!((m[1].score - -10.0).abs() < 1e-5, "score {}", m[1].score);
    assert_eq!(mock.seen()[0].body["distance_metric"], json!("euclidean_squared"));
}

#[valtron_test]
async fn dot_metric_is_unsupported() {
    let client: DynNetClient = HttpClientBuilder::new().build();
    let err = TurboPufferVectorStore::with_client(
        client,
        "k".into(),
        VectorStoreConfig::new(3, DistanceMetric::Dot),
    )
    .err()
    .expect("dot is refused");
    assert!(matches!(err, VectorStoreError::Unsupported(_)));
}

#[valtron_test]
async fn new_builds_the_default_client() {
    let store =
        TurboPufferVectorStore::new("k".into(), VectorStoreConfig::new(3, DistanceMetric::L2))
            .expect("cosine and L2 are accepted");
    assert_eq!(store.config().metric, DistanceMetric::L2);

    let err = TurboPufferVectorStore::new("k".into(), VectorStoreConfig::new(3, DistanceMetric::Dot))
        .err()
        .expect("dot is refused");
    assert!(matches!(err, VectorStoreError::Unsupported(_)));
}

#[valtron_test]
async fn error_status_is_an_error_with_a_bounded_message() {
    let (mock, store) = make_store().await;
    let store = store.with_limits(TurboPufferLimits::default().with_max_error_body(10));
    mock.state.lock().unwrap().fail_with = Some((500, "x".repeat(1000)));

    let err = store.len_async("ns").await.unwrap_err();
    let VectorStoreError::Backend(msg) = err else {
        panic!("expected a backend error, got {err:?}");
    };
    assert!(msg.starts_with("HTTP 500: "), "{msg}");
    assert_eq!(msg.len(), "HTTP 500: ".len() + 10, "only 10 bytes read: {msg}");
}

#[valtron_test]
async fn oversized_response_is_an_error() {
    let (_mock, store) = make_store().await;
    let store = store.with_limits(TurboPufferLimits::default().with_max_response_body(8));
    // A write reply is longer than 8 bytes.
    let err = store
        .insert_async("ns", entry("a", [1.0, 0.0, 0.0]))
        .await
        .unwrap_err();
    assert!(
        matches!(&err, VectorStoreError::Backend(m) if m.contains("max_response_body")),
        "{err:?}"
    );
}
