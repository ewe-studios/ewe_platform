//! Mock-D1 test for `D1Store::init` (the `StateStore` multi-statement path).
//!
//! E6: the `/query` body must carry every `init` statement with its `;` —
//! the CREATE TABLE plus the four CREATE INDEX statements — not just the first
//! statement. The mock captures the SQL so the test can assert on it.

#![cfg(feature = "d1")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use foundation_core::valtron::valtron_test;
use foundation_db::state::StateStore;
use foundation_db::D1Store;
use serde_json::json;

type Captured = Arc<Mutex<Option<String>>>;

fn respond(stream: &mut TcpStream, status: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    stream.flush().unwrap();
}

fn handle(mut stream: TcpStream, captured: &Captured) {
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
    *captured.lock().unwrap() = Some(req["sql"].as_str().unwrap_or_default().to_string());

    let reply = json!({
        "result": [{ "results": {"columns": [], "rows": []}, "success": true, "meta": {} }],
        "success": true,
        "errors": [],
        "messages": [],
    });
    respond(&mut stream, "200 OK", reply.to_string().as_bytes());
}

/// Start the mock and return a state-mode store, plus the captured `/query` SQL.
fn mock_store() -> (D1Store, Captured) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let captured: Captured = Arc::default();
    let served = Arc::clone(&captured);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            let served = Arc::clone(&served);
            thread::spawn(move || handle(stream, &served));
        }
    });
    let store = D1Store::new_state_with_base_url("token", "acct", "db", "proj", "dev", &base);
    (store, captured)
}

#[valtron_test]
async fn init_sends_every_statement_with_semicolon() {
    let (store, captured) = mock_store();
    store.init().await.unwrap();

    let sql = captured
        .lock()
        .unwrap()
        .clone()
        .expect("the mock saw one /query call");
    let statements: Vec<&str> = sql
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    // CREATE TABLE + four CREATE INDEX statements, all present and separated.
    assert_eq!(statements.len(), 5);
    assert!(statements[0].starts_with("CREATE TABLE IF NOT EXISTS proj_dev_resources"));
    for stmt in &statements[1..] {
        assert!(stmt.starts_with("CREATE INDEX IF NOT EXISTS idx_proj_dev_resources"));
    }
}
