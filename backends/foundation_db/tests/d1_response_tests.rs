//! `D1Store` request values and response bodies against an in-process mock of
//! the D1 REST `/query` API: non-finite REAL values are rejected, and a
//! response body is read under a configured cap.

#![cfg(feature = "d1")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use foundation_core::valtron::valtron_test;
use foundation_db::{D1Store, DataValue, QueryStore, StorageError};
use futures_lite::StreamExt;
use serde_json::json;

/// What the mock answers with: a status line and a body.
#[derive(Clone)]
struct Reply {
    status: &'static str,
    body: String,
}

fn respond(stream: &mut TcpStream, reply: &Reply) {
    let head = format!(
        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        reply.status,
        reply.body.len()
    );
    // The client may stop reading an over-cap body early; a failed write is fine.
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(reply.body.as_bytes());
    let _ = stream.flush();
}

/// Read one HTTP request (head and body) and drop it.
fn consume_request(stream: &mut TcpStream) -> bool {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap() == 0 {
        return false;
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
    true
}

/// A mock that answers every request with `reply`; returns the store and the
/// number of requests the mock has received.
fn mock_store(reply: Reply) -> (D1Store, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&seen);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let reply = reply.clone();
            let counter = Arc::clone(&counter);
            thread::spawn(move || {
                if consume_request(&mut stream) {
                    counter.fetch_add(1, Ordering::SeqCst);
                    respond(&mut stream, &reply);
                }
            });
        }
    });
    (
        D1Store::new_kv_with_base_url("token", "acct", "db", "app", &base),
        seen,
    )
}

/// A successful `/query` body with one row whose `note` column holds `padding`
/// characters.
fn rows_body(padding: usize) -> String {
    json!({
        "success": true,
        "errors": [],
        "messages": [],
        "result": [{
            "success": true,
            "meta": {},
            "results": [{ "id": 1, "note": "x".repeat(padding) }],
        }],
    })
    .to_string()
}

fn ok(body: String) -> Reply {
    Reply {
        status: "200 OK",
        body,
    }
}

#[valtron_test]
async fn d1_rejects_non_finite_real() {
    let (store, seen) = mock_store(ok(rows_body(0)));

    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let err = store
            .execute("INSERT INTO t (v) VALUES (?)", &[DataValue::Real(bad)])
            .await
            .unwrap_err();
        assert!(
            matches!(&err, StorageError::SqlConversion(msg) if msg.contains("non-finite")),
            "{bad}: expected a non-finite REAL error, got {err:?}"
        );
    }
    assert_eq!(
        seen.load(Ordering::SeqCst),
        0,
        "a rejected value must not reach the network"
    );

    // A batch rejects it too, before any request.
    let err = store
        .execute_batch_statements(&[(
            "INSERT INTO t (v) VALUES (?)".to_string(),
            vec![DataValue::Real(f64::NAN)],
        )])
        .await
        .unwrap_err();
    assert!(matches!(err, StorageError::SqlConversion(_)), "{err:?}");
    assert_eq!(seen.load(Ordering::SeqCst), 0);

    // A finite REAL still goes through.
    let mut rows = store
        .query("SELECT 1", &[DataValue::Real(1.5)])
        .await
        .unwrap();
    assert!(rows.next().await.is_some());
    assert_eq!(seen.load(Ordering::SeqCst), 1);
}

async fn first_row_note(store: &D1Store) -> Result<String, StorageError> {
    let mut rows = store.query("SELECT 1", &[]).await?;
    let row = rows.next().await.expect("one row")?;
    row.get_by_name::<String>("note")
}

#[valtron_test]
async fn d1_response_over_cap_is_an_error() {
    // Far over the cap: a typed error, not a partial parse.
    let (store, _) = mock_store(ok(rows_body(100_000)));
    let store = store.with_max_response_body(4096);
    let err = first_row_note(&store).await.unwrap_err();
    assert!(
        matches!(&err, StorageError::Backend(msg) if msg.contains("max_response_body")),
        "expected a response-too-large error, got {err:?}"
    );

    // Under the cap: parsed as before.
    let (store, _) = mock_store(ok(rows_body(100)));
    let store = store.with_max_response_body(4096);
    assert_eq!(first_row_note(&store).await.unwrap(), "x".repeat(100));
}

#[valtron_test]
async fn d1_response_cap_is_inclusive() {
    let body = rows_body(500);
    let (store, _) = mock_store(ok(body.clone()));
    let at_cap = store.with_max_response_body(body.len());
    assert_eq!(first_row_note(&at_cap).await.unwrap(), "x".repeat(500));

    let (store, _) = mock_store(ok(body.clone()));
    let one_short = store.with_max_response_body(body.len() - 1);
    assert!(first_row_note(&one_short).await.is_err());
}

#[valtron_test]
async fn d1_error_body_is_read_up_to_its_cap_only() {
    let (store, _) = mock_store(Reply {
        status: "500 Internal Server Error",
        body: format!("BOOM{}", "E".repeat(1_000_000)),
    });
    let store = store.with_max_error_body(16);
    let err = first_row_note(&store).await.unwrap_err();
    let StorageError::Backend(msg) = &err else {
        panic!("expected a Backend error, got {err:?}");
    };
    assert!(msg.contains("500"), "status in the message: {msg}");
    assert!(
        msg.contains("BOOM"),
        "start of the body in the message: {msg}"
    );
    assert!(
        msg.len() < 200,
        "the body is bounded, got {} bytes",
        msg.len()
    );
}
