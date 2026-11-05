//! `R2Store` as a `StateStore`, against an in-process mock of the Cloudflare
//! R2 REST API (object GET/PUT/DELETE and the paged object listing).
//!
//! The mock pages listings two keys at a time, so every listing here spans
//! several pages and exercises the cursor loop.

#![cfg(feature = "r2")]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use chrono::{Duration, Utc};
use foundation_core::valtron::valtron_test;
use foundation_core::valtron::BlockingFutureExt;
use foundation_db::core::state::traits::StateStoreStream;
use foundation_db::state::{ResourceState, StateStatus, StateStore};
use foundation_db::R2Store;
use futures_lite::StreamExt;
use serde_json::json;

/// Keys per listing page in the mock.
const PAGE: usize = 2;

type Objects = Arc<Mutex<BTreeMap<String, Vec<u8>>>>;
type ListRequests = Arc<AtomicUsize>;

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap();
            out.push(u8::from_str_radix(hex, 16).unwrap());
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

fn respond(stream: &mut TcpStream, status: &str, body: &[u8]) {
    respond_etag(stream, status, body, None);
}

/// Reply, optionally with an `ETag` header.
fn respond_etag(stream: &mut TcpStream, status: &str, body: &[u8], etag: Option<&str>) {
    let etag = etag.map_or_else(String::new, |value| format!("ETag: {value}\r\n"));
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{etag}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    stream.flush().unwrap();
}

/// A stable, quoted ETag for an object body, like R2 returns for the object PUT.
fn etag_of(bytes: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("\"{hash:016x}\"")
}

fn handle(mut stream: TcpStream, objects: &Objects, list_requests: &ListRequests) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap() == 0 {
        return;
    }
    let mut content_length = 0usize;
    let mut expects_continue = false;
    let mut if_match: Option<String> = None;
    let mut if_none_match: Option<String> = None;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap();
            }
            if name.eq_ignore_ascii_case("expect")
                && value.trim().eq_ignore_ascii_case("100-continue")
            {
                expects_continue = true;
            }
            if name.eq_ignore_ascii_case("if-match") {
                if_match = Some(value.trim().to_string());
            }
            if name.eq_ignore_ascii_case("if-none-match") {
                if_none_match = Some(value.trim().to_string());
            }
        }
    }
    // Answer `Expect: 100-continue` like a real server, or the client waits
    // out its continue timeout before sending the body.
    if expects_continue {
        stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").unwrap();
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).unwrap();

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap().to_string();
    let target = parts.next().unwrap().to_string();
    let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));
    let (_, object_path) = path.split_once("/objects").unwrap();

    let mut objects = objects.lock().unwrap();
    match (method.as_str(), object_path) {
        ("GET", "") => {
            list_requests.fetch_add(1, Ordering::SeqCst);
            let params: BTreeMap<&str, String> = query
                .split('&')
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (k, percent_decode(v)))
                .collect();
            let prefix = params.get("prefix").cloned().unwrap_or_default();
            let start: usize = params.get("cursor").map_or(0, |c| c.parse().unwrap());
            let keys: Vec<&String> = objects.keys().filter(|k| k.starts_with(&prefix)).collect();
            let page: Vec<_> = keys
                .iter()
                .skip(start)
                .take(PAGE)
                .map(|k| json!({ "key": k }))
                .collect();
            let truncated = start + PAGE < keys.len();
            let reply = json!({
                "success": true,
                "errors": [],
                "messages": [],
                "result": page,
                "result_info": {
                    "is_truncated": truncated,
                    "cursor": if truncated { (start + PAGE).to_string() } else { String::new() },
                },
            });
            respond(&mut stream, "200 OK", reply.to_string().as_bytes());
        }
        ("GET", key) => match objects.get(&percent_decode(&key[1..])) {
            Some(data) => {
                let etag = etag_of(data);
                respond_etag(&mut stream, "200 OK", &data.clone(), Some(&etag));
            }
            None => respond(&mut stream, "404 Not Found", b"{}"),
        },
        ("PUT", key) => {
            let key = percent_decode(&key[1..]);
            let current = objects.get(&key).map(|data| etag_of(data));
            let precondition_failed = if_none_match.as_deref() == Some("*") && current.is_some()
                || if_match
                    .as_deref()
                    .is_some_and(|expected| current.as_deref() != Some(expected));
            if precondition_failed {
                respond(&mut stream, "412 Precondition Failed", b"{}");
            } else {
                let etag = etag_of(&body);
                objects.insert(key, body);
                respond_etag(&mut stream, "200 OK", b"{\"success\":true}", Some(&etag));
            }
        }
        ("DELETE", key) => {
            objects.remove(&percent_decode(&key[1..]));
            respond(&mut stream, "200 OK", b"{\"success\":true}");
        }
        _ => respond(&mut stream, "405 Method Not Allowed", b"{}"),
    }
}

/// Start the mock and return a state-mode store pointed at it, plus the
/// object map so tests can see what landed in the bucket.
fn mock_store() -> (R2Store, Objects) {
    let (store, objects, _list_requests) = mock_store_with_counter();
    (store, objects)
}

/// Like [`mock_store`], but also returns a counter of object-listing requests.
fn mock_store_with_counter() -> (R2Store, Objects, ListRequests) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let objects: Objects = Arc::default();
    let served = Arc::clone(&objects);
    let list_requests: ListRequests = Arc::default();
    let served_requests = Arc::clone(&list_requests);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            let served = Arc::clone(&served);
            let served_requests = Arc::clone(&served_requests);
            thread::spawn(move || handle(stream, &served, &served_requests));
        }
    });
    let store = R2Store::new_state_with_base_url("token", "acct", "bucket", "proj", "dev", &base);
    (store, objects, list_requests)
}

fn state(id: &str, kind: &str) -> ResourceState {
    let now = Utc::now();
    ResourceState {
        id: id.to_string(),
        kind: kind.to_string(),
        provider: "test".to_string(),
        status: StateStatus::Created,
        environment: None,
        config_hash: String::new(),
        output: json!({ "id": id }),
        config_snapshot: json!(null),
        created_at: now,
        updated_at: now,
    }
}

async fn collect<T>(stream: StateStoreStream<T>) -> Vec<T> {
    stream.map(Result::unwrap).collect().await
}

async fn seeded() -> (R2Store, Objects) {
    let (store, objects) = mock_store();
    store.init().await.unwrap();
    for (id, kind) in [
        ("a", "worker"),
        ("b", "bucket"),
        ("ns/c", "worker"),
        ("ns/d", "bucket"),
        ("ns/e", "worker"),
    ] {
        store.set(id, &state(id, kind)).await.unwrap();
    }
    (store, objects)
}

#[valtron_test]
async fn set_writes_json_under_project_stage_key() {
    let (store, objects) = mock_store();
    store.set("ns/x", &state("ns/x", "worker")).await.unwrap();

    let keys: Vec<String> = objects.lock().unwrap().keys().cloned().collect();
    assert_eq!(keys, vec!["proj/dev/ns:x.json".to_string()]);

    let got = store.get("ns/x").await.unwrap().unwrap();
    assert_eq!(got.id, "ns/x");
    assert_eq!(got.kind, "worker");
}

#[valtron_test]
async fn set_uses_resource_id() {
    let (store, _objects) = mock_store();

    // The caller labels the state "b" but writes it under "a".
    let state_with_id_b = state("b", "worker");
    store.set("a", &state_with_id_b).await.unwrap();

    assert_eq!(store.get("a").await.unwrap().unwrap().id, "a");
    assert!(store.get("b").await.unwrap().is_none());
}

#[valtron_test]
async fn get_missing_is_none_and_delete_missing_is_ok() {
    let (store, _objects) = mock_store();
    assert!(store.get("nope").await.unwrap().is_none());
    store.delete("nope").await.unwrap();
}

#[valtron_test]
async fn list_and_count_follow_every_page() {
    let (store, _objects) = seeded().await;

    let ids = collect(store.list().await.unwrap()).await;
    assert_eq!(ids, vec!["a", "b", "ns/c", "ns/d", "ns/e"]);
    assert_eq!(store.count().await.unwrap(), 5);
}

#[valtron_test]
async fn prefix_operations_match_slash_ids() {
    let (store, _objects) = seeded().await;

    let ids = collect(store.list_by_prefix("ns/").await.unwrap()).await;
    assert_eq!(ids, vec!["ns/c", "ns/d", "ns/e"]);
    assert_eq!(store.count_by_prefix("ns/").await.unwrap(), 3);

    let states = collect(store.all_by_prefix("ns/").await.unwrap()).await;
    let ids: Vec<&str> = states.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["ns/c", "ns/d", "ns/e"]);

    assert_eq!(store.delete_by_prefix("ns/").await.unwrap(), 3);
    assert_eq!(collect(store.list().await.unwrap()).await, vec!["a", "b"]);
}

#[valtron_test]
async fn all_and_batch_skip_missing_ids() {
    let (store, _objects) = seeded().await;

    assert_eq!(collect(store.all().await.unwrap()).await.len(), 5);

    let batch = collect(store.get_batch(&["a", "missing", "ns/e"]).await.unwrap()).await;
    let ids: Vec<&str> = batch.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["a", "ns/e"]);
}

#[valtron_test]
async fn find_by_kind_filters() {
    let (store, _objects) = seeded().await;

    let workers = collect(store.find_by_kind("worker").await.unwrap()).await;
    let ids: Vec<&str> = workers.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["a", "ns/c", "ns/e"]);

    let created = collect(
        store
            .find_by_status(&StateStatus::Created.to_string())
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(created.len(), 5);
    assert!(collect(store.find_by_provider("other").await.unwrap())
        .await
        .is_empty());
}

#[valtron_test]
async fn r2_state_listing_streams_pages() {
    let (store, _objects, list_requests) = mock_store_with_counter();
    store.init().await.unwrap();
    for id in ["a", "b", "c", "d", "e"] {
        store.set(id, &state(id, "worker")).await.unwrap();
    }

    let mut stream = store.list().await.unwrap();

    // Pull the first id: only the first page (2 keys) has been requested.
    let first = stream.next().await.unwrap().unwrap();
    assert_eq!(first, "a");
    assert_eq!(list_requests.load(Ordering::SeqCst), 1);

    // The remaining ids come from pages 2 and 3, fetched lazily.
    let rest = collect(stream).await;
    assert_eq!(rest, vec!["b", "c", "d", "e"]);
    assert_eq!(list_requests.load(Ordering::SeqCst), 3);
}

// ===========================================================================
// StateStore::set_if_version — R2 conditional object PUT
// ===========================================================================

/// The contract, one writer: absent-only, stale version and current version.
async fn check_conditional_contract(store: &R2Store) {
    let first = state("r1", "v1");

    assert!(!store
        .set_if_version("r1", &first, Some(first.updated_at))
        .await
        .unwrap());
    assert!(store.set_if_version("r1", &first, None).await.unwrap());
    assert!(!store.set_if_version("r1", &first, None).await.unwrap());

    let stored = store.get("r1").await.unwrap().unwrap();
    let mut second = state("r1", "v2");
    second.updated_at = stored.updated_at + Duration::seconds(1);
    assert!(!store
        .set_if_version(
            "r1",
            &second,
            Some(stored.updated_at - Duration::seconds(5)),
        )
        .await
        .unwrap());
    assert_eq!(store.get("r1").await.unwrap().unwrap().kind, "v1");
    assert!(store
        .set_if_version("r1", &second, Some(stored.updated_at))
        .await
        .unwrap());
    assert_eq!(store.get("r1").await.unwrap().unwrap().kind, "v2");

    // A stale writer (it still holds the first version) loses.
    let mut stale = state("r1", "stale");
    stale.updated_at = stored.updated_at + Duration::seconds(2);
    assert!(!store
        .set_if_version("r1", &stale, Some(stored.updated_at))
        .await
        .unwrap());
    assert_eq!(store.get("r1").await.unwrap().unwrap().kind, "v2");
}

#[valtron_test(threads = 8, timeout = 60000)]
fn set_if_version_uses_the_conditional_put() {
    async move {
        let (store, _objects) = mock_store();
        store.init().await.unwrap();
        check_conditional_contract(&store).await;
    }
    .block();
}

/// 20 writers read the same version and all try to write it: exactly one wins.
/// The mock honours `If-None-Match: *` and `If-Match`, like the real R2 binding.
#[valtron_test(threads = 8, timeout = 60000)]
fn set_if_version_race_has_one_winner() {
    const THREADS: usize = 20;

    let (store, seed) = async move {
        let (store, _objects) = mock_store();
        store.init().await.unwrap();
        let seed = state("race", "seed");
        store.set("race", &seed).await.unwrap();
        let seed = store.get("race").await.unwrap().unwrap();
        (store, seed)
    }
    .block();
    let store = std::sync::Arc::new(store);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|n| {
            let (store, barrier, seed) = (
                std::sync::Arc::clone(&store),
                std::sync::Arc::clone(&barrier),
                seed.clone(),
            );
            thread::spawn(move || {
                barrier.wait();
                async move {
                    let mut mine = state("race", &format!("writer-{n}"));
                    mine.updated_at = seed.updated_at + Duration::seconds(1);
                    store
                        .set_if_version("race", &mine, Some(seed.updated_at))
                        .await
                        .unwrap()
                }
                .block()
            })
        })
        .collect();
    let winners = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .filter(|won| *won)
        .count();
    assert_eq!(winners, 1, "exactly one writer wins, not {winners}");
}
