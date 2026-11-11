//! Contract tests for `BlobStore`'s range, conditional-put and ordered
//! listing APIs, run identically on the memory, JSON-file and R2 (in-process
//! mock) backends.
//!
//! E4 fixes `BlobStore` so objects move as streams, not whole buffers:
//! streamed get/put, byte ranges, conditional put (`If-None-Match: *`,
//! `If-Match: etag`) and ordered listings. These five tests are the contract
//! every backend must satisfy.

#![cfg(feature = "r2")]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use foundation_core::valtron::valtron_test;
use foundation_db::traits::{
    content_etag, BlobListStream, BlobPutCondition, BlobRange, BlobStore, BlobStream,
};
use foundation_db::{JsonFileStorage, MemoryStorage, R2Store, StorageError};
use futures_lite::StreamExt;

/// A backend under test: the store plus any lifetime guard (the JSON-file
/// backend's temporary directory).
struct Backend {
    store: Arc<dyn BlobStore>,
    _temp: Option<tempfile::TempDir>,
}

fn memory_backend() -> Backend {
    Backend {
        store: Arc::new(MemoryStorage::new()),
        _temp: None,
    }
}

fn file_backend() -> Backend {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.json");
    Backend {
        store: Arc::new(JsonFileStorage::new(&path).unwrap()),
        _temp: Some(dir),
    }
}

fn r2_backend() -> Backend {
    let (store, _objects) = mock_blob_store();
    Backend {
        store: Arc::new(store),
        _temp: None,
    }
}

/// A body source that counts the chunks it yields, so the test can prove the
/// backend pulled from the stream rather than asking for one whole buffer.
fn counting_stream(chunks: Vec<Vec<u8>>, counter: Arc<AtomicUsize>) -> BlobStream {
    Box::pin(
        futures_lite::stream::iter(chunks)
            .inspect(move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
            })
            .map(Ok::<Vec<u8>, StorageError>),
    )
}

async fn collect_bytes(mut stream: BlobStream) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(chunk) = stream.next().await {
        out.extend_from_slice(&chunk.unwrap());
    }
    out
}

async fn collect_keys(mut stream: BlobListStream) -> Vec<String> {
    let mut out = Vec::new();
    while let Some(key) = stream.next().await {
        out.push(key.unwrap());
    }
    out
}

macro_rules! blob_contract {
    ($mod:ident, $make:expr) => {
        mod $mod {
            use super::*;

            #[valtron_test]
            async fn blob_get_range() {
                let backend = $make();
                let store = &backend.store;

                store.put_blob("k", b"0123456789").await.unwrap();
                let range = BlobRange {
                    start: 2,
                    end: Some(5),
                };
                let stream = store.get_blob_range("k", range).await.unwrap();
                assert_eq!(collect_bytes(stream).await, b"234");

                // An unbounded end reads to the last byte.
                let tail = store
                    .get_blob_range(
                        "k",
                        BlobRange {
                            start: 8,
                            end: None,
                        },
                    )
                    .await
                    .unwrap();
                assert_eq!(collect_bytes(tail).await, b"89");

                // A missing object reads as an empty range.
                let missing = store
                    .get_blob_range(
                        "missing",
                        BlobRange {
                            start: 0,
                            end: Some(4),
                        },
                    )
                    .await
                    .unwrap();
                assert_eq!(collect_bytes(missing).await, Vec::<u8>::new());
            }

            #[valtron_test]
            async fn blob_put_if_none_match_rejects_existing() {
                let backend = $make();
                let store = &backend.store;

                store.put_blob("k", b"first").await.unwrap();
                let err = store
                    .put_blob_conditionally("k", b"second", BlobPutCondition::IfNoneMatch)
                    .await
                    .unwrap_err();
                assert!(matches!(err, StorageError::PreconditionFailed(_)));
                assert_eq!(
                    store.get_blob("k").await.unwrap().unwrap(),
                    b"first".to_vec()
                );
            }

            #[valtron_test]
            async fn blob_put_if_match_rejects_stale_etag() {
                let backend = $make();
                let store = &backend.store;

                store.put_blob("k", b"v1").await.unwrap();
                let etag = store.blob_etag("k").await.unwrap().expect("etag");

                // The content changes, so the remembered etag is now stale.
                store.put_blob("k", b"v2").await.unwrap();
                let err = store
                    .put_blob_conditionally("k", b"v3", BlobPutCondition::IfMatch(etag))
                    .await
                    .unwrap_err();
                assert!(matches!(err, StorageError::PreconditionFailed(_)));
                assert_eq!(store.get_blob("k").await.unwrap().unwrap(), b"v2".to_vec());
            }

            #[valtron_test]
            async fn blob_list_after_is_ordered() {
                let backend = $make();
                let store = &backend.store;

                for (key, value) in [
                    ("c", b"c".to_vec()),
                    ("a", b"a".to_vec()),
                    ("b", b"b".to_vec()),
                    ("d", b"d".to_vec()),
                ] {
                    store.put_blob(&key, &value).await.unwrap();
                }

                let all = collect_keys(store.list_blobs_after("", None, None).await.unwrap()).await;
                assert_eq!(all, vec!["a", "b", "c", "d"]);

                let after_b = collect_keys(
                    store
                        .list_blobs_after("", Some("b"), Some(2))
                        .await
                        .unwrap(),
                )
                .await;
                assert_eq!(after_b, vec!["c", "d"]);
            }

            #[valtron_test]
            async fn blob_put_streams_without_buffering() {
                let backend = $make();
                let store = &backend.store;

                let counter = Arc::new(AtomicUsize::new(0));
                let chunks = vec![b"abc".to_vec(), b"def".to_vec(), b"ghi".to_vec()];
                let length: u64 = chunks.iter().map(|c| c.len() as u64).sum();
                let stream = counting_stream(chunks, Arc::clone(&counter));

                store
                    .put_blob_stream("k", Some(length), stream)
                    .await
                    .unwrap();

                // Every chunk was pulled from the source exactly once.
                assert_eq!(counter.load(Ordering::SeqCst), 3);
                assert_eq!(
                    store.get_blob("k").await.unwrap().unwrap(),
                    b"abcdefghi".to_vec()
                );
            }
        }
    };
}

blob_contract!(memory_contract, memory_backend);
blob_contract!(file_contract, file_backend);
blob_contract!(r2_contract, r2_backend);

// ===========================================================================
// In-process R2 mock (object GET/HEAD/PUT/DELETE plus paged LIST, byte ranges
// and conditional writes).
// ===========================================================================

type Objects = Arc<Mutex<BTreeMap<String, Vec<u8>>>>;

/// Keys per listing page in the mock.
const PAGE: usize = 2;

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

fn respond_body(stream: &mut TcpStream, status: &str, extra: &[(&str, &str)], body: &[u8]) {
    let mut head = format!("HTTP/1.1 {status}\r\n");
    for (name, value) in extra {
        write!(head, "{name}: {value}\r\n").unwrap();
    }
    write!(
        head,
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    stream.flush().unwrap();
}

/// Read a chunked (`Transfer-Encoding: chunked`) request body into one buffer.
/// Each chunk is `{size-in-hex}\r\n{data}\r\n`; a zero-size chunk ends the
/// body, optionally followed by trailer lines.
fn read_chunked_body(reader: &mut BufReader<TcpStream>) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut size_line = String::new();
        if reader.read_line(&mut size_line).unwrap() == 0 {
            break;
        }
        let size_hex = size_line.trim().split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_hex, 16).unwrap_or(0);
        if size == 0 {
            // Trailer section ends at an empty line.
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" || line == "\n" {
                    break;
                }
            }
            break;
        }
        let mut chunk = vec![0; size];
        reader.read_exact(&mut chunk).unwrap();
        let mut crlf = [0u8; 2];
        reader.read_exact(&mut crlf).unwrap();
        out.extend_from_slice(&chunk);
    }
    out
}

/// `bytes=start-end` or `bytes=start-` → inclusive `[start, end]`, clamped to
/// the body, or `None` when the range is not satisfiable.
fn parse_range(header: &str, len: usize) -> Option<(usize, usize)> {
    let spec = header.strip_prefix("bytes=")?;
    let (start_s, end_s) = spec.split_once('-')?;
    let start: usize = start_s.parse().ok()?;
    let end = if end_s.is_empty() {
        len.saturating_sub(1)
    } else {
        end_s.parse().ok()?
    };
    if start >= len || start > end {
        return None;
    }
    Some((start, end.min(len - 1)))
}

// One request handler: it parses the request line, headers, range and body in sequence.
#[allow(clippy::too_many_lines)]
fn blob_handle(mut stream: TcpStream, objects: &Objects) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap() == 0 {
        return;
    }
    let mut content_length = 0usize;
    let mut expects_continue = false;
    let mut chunked = false;
    let mut range_header: Option<String> = None;
    let mut if_none_match: Option<String> = None;
    let mut if_match: Option<String> = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap() == 0 {
            break;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            match name.as_str() {
                "content-length" => content_length = value.parse().unwrap_or(0),
                "transfer-encoding" => chunked = value.to_ascii_lowercase().contains("chunked"),
                "expect" if value.eq_ignore_ascii_case("100-continue") => expects_continue = true,
                "range" => range_header = Some(value),
                "if-none-match" => if_none_match = Some(value),
                "if-match" => if_match = Some(value),
                _ => {}
            }
        }
    }
    if expects_continue {
        stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").unwrap();
    }
    let body = if chunked {
        read_chunked_body(&mut reader)
    } else {
        let mut body = vec![0; content_length];
        reader.read_exact(&mut body).unwrap();
        body
    };

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
            let page: Vec<_> = keys
                .iter()
                .skip(start)
                .take(PAGE)
                .map(|k| serde_json::json!({ "key": k }))
                .collect();
            let truncated = start + PAGE < keys.len();
            let reply = serde_json::json!({
                "success": true,
                "errors": [],
                "messages": [],
                "result": page,
                "result_info": {
                    "is_truncated": truncated,
                    "cursor": if truncated { (start + PAGE).to_string() } else { String::new() },
                },
            });
            respond_body(
                &mut stream,
                "200 OK",
                &[("Content-Type", "application/json")],
                reply.to_string().as_bytes(),
            );
        }
        ("HEAD", key) => {
            let key = percent_decode(&key[1..]);
            match objects.get(&key) {
                Some(data) => {
                    respond_body(&mut stream, "200 OK", &[("etag", &content_etag(data))], b"");
                }
                None => respond_body(&mut stream, "404 Not Found", &[], b""),
            }
        }
        ("GET", key) => {
            let key = percent_decode(&key[1..]);
            match objects.get(&key) {
                None => respond_body(&mut stream, "404 Not Found", &[], b"{}"),
                Some(data) => match range_header
                    .as_deref()
                    .and_then(|h| parse_range(h, data.len()))
                {
                    Some((start, end)) => {
                        respond_body(&mut stream, "206 Partial Content", &[], &data[start..=end]);
                    }
                    None => respond_body(&mut stream, "200 OK", &[], data),
                },
            }
        }
        ("PUT", key) => {
            let key = percent_decode(&key[1..]);
            // Conditional writes: reject on precondition failure before storing.
            if let Some(value) = &if_none_match {
                if value == "*" && objects.contains_key(&key) {
                    respond_body(&mut stream, "412 Precondition Failed", &[], b"{}");
                    return;
                }
            }
            if let Some(expected) = &if_match {
                match objects.get(&key) {
                    Some(data) if content_etag(data) == *expected => {}
                    _ => {
                        respond_body(&mut stream, "412 Precondition Failed", &[], b"{}");
                        return;
                    }
                }
            }
            objects.insert(key, body);
            respond_body(&mut stream, "200 OK", &[], b"{\"success\":true}");
        }
        ("DELETE", key) => {
            let key = percent_decode(&key[1..]);
            objects.remove(&key);
            respond_body(&mut stream, "200 OK", &[], b"{\"success\":true}");
        }
        _ => respond_body(&mut stream, "405 Method Not Allowed", &[], b"{}"),
    }
}

/// Start the mock and return a blob-mode `R2Store` pointed at it, plus the
/// object map so tests can inspect what landed in the bucket.
fn mock_blob_store() -> (R2Store, Objects) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let objects: Objects = Arc::default();
    let served = Arc::clone(&objects);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            let served = Arc::clone(&served);
            thread::spawn(move || blob_handle(stream, &served));
        }
    });
    let store = R2Store::new_blob_with_base_url("token", "acct", "bucket", "blobs", &base);
    (store, objects)
}
