//! `D1Store::execute_batch_statements` against an in-process mock of the D1
//! REST `/query` API. The mock applies a multi-statement batch only when every
//! statement succeeds, matching D1's all-or-nothing batch contract.

#![cfg(feature = "d1")]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use foundation_core::valtron::valtron_test;
use foundation_db::{D1Store, QueryStore, StorageError};
use serde_json::json;

type Rows = Arc<Mutex<BTreeMap<String, String>>>;

fn respond(stream: &mut TcpStream, status: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    stream.flush().unwrap();
}

/// Split the joined SQL and record the literal `INSERT INTO batch_test` rows.
/// Any statement the mock does not recognise fails the whole batch, and then
/// nothing is applied.
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
            if name.eq_ignore_ascii_case("expect")
                && value.trim().eq_ignore_ascii_case("100-continue")
            {
                expects_continue = true;
            }
        }
    }
    if expects_continue {
        stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").unwrap();
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).unwrap();

    let req: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let sql = req["sql"].as_str().unwrap_or_default();
    let statements: Vec<&str> = sql
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    let mut pending: Vec<(String, String)> = Vec::new();
    let mut failed = false;
    for stmt in &statements {
        if let Some(values) = stmt.strip_prefix("INSERT INTO batch_test (key, value) VALUES") {
            let values = values.trim().trim_start_matches('(').trim_end_matches(')');
            let parts: Vec<&str> = values.split(',').map(str::trim).collect();
            if parts.len() == 2 {
                pending.push((
                    parts[0].trim_matches('\'').to_string(),
                    parts[1].trim_matches('\'').to_string(),
                ));
            } else {
                failed = true;
            }
        } else if stmt.starts_with("CREATE") {
            // Schema statement: succeeds with no rows.
        } else {
            failed = true;
        }
    }

    let reply = if failed {
        json!({
            "success": false,
            "errors": [{ "code": 7500, "message": "statement failed" }],
            "messages": [],
            "result": [],
        })
    } else {
        let mut map = rows.lock().unwrap();
        for (key, value) in pending {
            map.insert(key, value);
        }
        json!({
            "success": true,
            "errors": [],
            "messages": [],
            "result": statements.iter().map(|_| json!({ "results": [], "success": true, "meta": {} })).collect::<Vec<_>>(),
        })
    };
    respond(&mut stream, "200 OK", reply.to_string().as_bytes());
}

/// Start the mock and return a KV-mode store pointed at it, plus the applied
/// rows so the test can see what committed.
fn mock_store() -> (D1Store, Rows) {
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
    let store = D1Store::new_kv_with_base_url("token", "acct", "db", "app", &base);
    (store, rows)
}

#[valtron_test]
async fn batch_statements_apply_atomically() {
    let (store, rows) = mock_store();

    let failing = vec![
        (
            "INSERT INTO batch_test (key, value) VALUES ('a', '1')".to_string(),
            vec![],
        ),
        (
            "INSERT INTO batch_test (key, value) VALUES ('b', '2')".to_string(),
            vec![],
        ),
        (
            "INSERT INTO SHOULD_FAIL (key, value) VALUES ('c', '3')".to_string(),
            vec![],
        ),
    ];
    let err = store.execute_batch_statements(&failing).await.unwrap_err();
    assert!(matches!(err, StorageError::Backend(_)));
    assert!(
        rows.lock().unwrap().is_empty(),
        "none of the three statements may apply"
    );
}

#[valtron_test]
async fn batch_statements_apply_when_all_succeed() {
    let (store, rows) = mock_store();

    let ok = vec![
        (
            "INSERT INTO batch_test (key, value) VALUES ('a', '1')".to_string(),
            vec![],
        ),
        (
            "INSERT INTO batch_test (key, value) VALUES ('b', '2')".to_string(),
            vec![],
        ),
    ];
    store.execute_batch_statements(&ok).await.unwrap();

    let map = rows.lock().unwrap();
    assert_eq!(map.get("a").map(String::as_str), Some("1"));
    assert_eq!(map.get("b").map(String::as_str), Some("2"));
}
