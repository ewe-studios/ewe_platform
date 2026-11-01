//! `D1Store` as a `StateStore`, against an in-process mock of the Cloudflare
//! D1 REST `/query` API (multi-statement `init`, upsert, and point SELECTs).
//!
//! The mock stores one row per `id` from the upsert params and answers
//! `SELECT … WHERE id = ?` from that map, so the store logic runs with no
//! external infrastructure. It keeps the same response shape the real D1 API
//! returns (`{ result: [{ results, success, meta }] }`).

#![cfg(feature = "d1")]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use chrono::Utc;
use foundation_core::valtron::valtron_test;
use foundation_db::state::{ResourceState, StateStatus, StateStore};
use foundation_db::D1Store;
use serde_json::json;

type Rows = Arc<Mutex<BTreeMap<String, serde_json::Value>>>;

fn respond(stream: &mut TcpStream, status: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    stream.flush().unwrap();
}

/// A row object built from the ten upsert params, in `state_to_params` order,
/// shaped the way `parse_d1_row` reads it.
fn row_from_params(params: &[serde_json::Value]) -> serde_json::Value {
    json!({
        "id": params[0],
        "kind": params[1],
        "provider": params[2],
        "status": params[3],
        "environment": params[4],
        "config_hash": params[5],
        "output": params[6],
        "config_snapshot": params[7],
        "created_at": params[8],
        "updated_at": params[9],
    })
}

fn handle(mut stream: TcpStream, rows: &Rows) {
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

    let req: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let sql = req["sql"].as_str().unwrap_or_default();
    let params = req["params"].as_array().cloned().unwrap_or_default();

    let reply = if sql.starts_with("INSERT") {
        let id = params[0].as_str().unwrap().to_string();
        rows.lock().unwrap().insert(id, row_from_params(&params));
        json!({
            "result": [{ "results": [], "success": true, "meta": {} }],
            "success": true,
            "errors": [],
            "messages": [],
        })
    } else if sql.starts_with("SELECT") && params.len() == 1 {
        let id = params[0].as_str().unwrap();
        let results = rows
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .map(|row| vec![row])
            .unwrap_or_default();
        json!({
            "result": [{ "results": results, "success": true, "meta": {} }],
            "success": true,
            "errors": [],
            "messages": [],
        })
    } else {
        // `init` (CREATE TABLE / CREATE INDEX) and anything else the test does
        // not assert on: succeed with no rows.
        json!({
            "result": [{ "results": [], "success": true, "meta": {} }],
            "success": true,
            "errors": [],
            "messages": [],
        })
    };
    respond(&mut stream, "200 OK", reply.to_string().as_bytes());
}

/// Start the mock and return a state-mode store pointed at it.
fn mock_store() -> D1Store {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let rows: Rows = Arc::default();
    let served = Arc::clone(&rows);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            let served = Arc::clone(&served);
            thread::spawn(move || handle(stream, &served));
        }
    });
    D1Store::new_state_with_base_url("token", "acct", "db", "proj", "dev", &base)
}

fn state(id: &str) -> ResourceState {
    let now = Utc::now();
    ResourceState {
        id: id.to_string(),
        kind: "worker".to_string(),
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

#[valtron_test]
async fn set_uses_resource_id() {
    let store = mock_store();

    // The caller labels the state "b" but writes it under "a".
    let state_with_id_b = state("b");
    store.set("a", &state_with_id_b).await.unwrap();

    assert_eq!(store.get("a").await.unwrap().unwrap().id, "a");
    assert!(store.get("b").await.unwrap().is_none());
}
