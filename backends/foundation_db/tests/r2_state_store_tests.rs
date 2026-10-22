//! `R2Store` as a `StateStore`, against an in-process mock of the Cloudflare
//! R2 REST API (object GET/PUT/DELETE and the paged object listing).
//!
//! The mock pages listings two keys at a time, so every listing here spans
//! several pages and exercises the cursor loop.

#![cfg(feature = "r2")]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use chrono::Utc;
use foundation_core::valtron::valtron_test;
use foundation_db::core::state::traits::StateStoreStream;
use foundation_db::state::{ResourceState, StateStatus, StateStore};
use foundation_db::R2Store;
use futures_lite::StreamExt;
use serde_json::json;

/// Keys per listing page in the mock.
const PAGE: usize = 2;

type Objects = Arc<Mutex<BTreeMap<String, Vec<u8>>>>;

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
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    stream.flush().unwrap();
}

fn handle(mut stream: TcpStream, objects: &Objects) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap() == 0 {
        return;
    }
    let mut content_length = 0usize;
    let mut expects_continue = false;
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
            if name.eq_ignore_ascii_case("expect") && value.trim().eq_ignore_ascii_case("100-continue") {
                expects_continue = true;
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
            let params: BTreeMap<&str, String> = query
                .split('&')
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (k, percent_decode(v)))
                .collect();
            let prefix = params.get("prefix").cloned().unwrap_or_default();
            let start: usize = params.get("cursor").map_or(0, |c| c.parse().unwrap());
            let keys: Vec<&String> = objects.keys().filter(|k| k.starts_with(&prefix)).collect();
            let page: Vec<_> = keys.iter().skip(start).take(PAGE).map(|k| json!({ "key": k })).collect();
            let truncated = start + PAGE < keys.len();
            let reply = json!({
                "success": true,
                "result": page,
                "result_info": {
                    "is_truncated": truncated,
                    "cursor": if truncated { (start + PAGE).to_string() } else { String::new() },
                },
            });
            respond(&mut stream, "200 OK", reply.to_string().as_bytes());
        }
        ("GET", key) => match objects.get(&percent_decode(&key[1..])) {
            Some(data) => respond(&mut stream, "200 OK", &data.clone()),
            None => respond(&mut stream, "404 Not Found", b"{}"),
        },
        ("PUT", key) => {
            objects.insert(percent_decode(&key[1..]), body);
            respond(&mut stream, "200 OK", b"{\"success\":true}");
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
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let objects: Objects = Arc::default();
    let served = Arc::clone(&objects);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            let served = Arc::clone(&served);
            thread::spawn(move || handle(stream, &served));
        }
    });
    let store = R2Store::new_state_with_base_url("token", "acct", "bucket", "proj", "dev", &base);
    (store, objects)
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

    let created = collect(store.find_by_status(&StateStatus::Created.to_string()).await.unwrap()).await;
    assert_eq!(created.len(), 5);
    assert!(collect(store.find_by_provider("other").await.unwrap()).await.is_empty());
}
