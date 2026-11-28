//! Golden oracle for the Durable Streams HTTP surface.
//!
//! One fixed script drives the native adapter and compares a normalized view
//! of each response with a stored fixture. The view keeps the protocol only:
//!
//! - the status,
//! - `Stream-Next-Offset` (presence and value),
//! - `Stream-Up-To-Date` (presence and value),
//! - `Stream-Closed` (presence and value),
//! - `Content-Type`,
//! - `ETag` (presence only),
//! - `X-Error-Kind`,
//! - the body shape: empty, text, JSON or raw bytes.
//!
//! Transport headers such as `Date`, `Server`, `Content-Length` and the
//! connection details never enter a fixture. The security and cache headers
//! have their own oracles.
//!
//! The fixtures live in `tests/golden/durable_http/`, one file per step. A
//! missing fixture fails the test. To rewrite the fixtures after a deliberate
//! protocol change, run:
//!
//! ```sh
//! UPDATE_GOLDEN=1 cargo test --profile uat -p foundation_http --features multi golden_durable_http
//! ```
//!
//! Then review the diff in the fixture files before you commit it.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use foundation_context::Ctx;
use foundation_core::durable::{DurableService, DurableStream, MemoryVfs, StreamName};
use foundation_http::native::durable::{dispatch, error_response, DurableRequest, DurableResponse};
use foundation_http::shared::durable::header;
use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders, SimpleMethod};
use serde_json::{json, Value};

/// The directory that holds the golden fixtures.
fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("durable_http")
}

fn service() -> DurableService<DurableStream> {
    DurableService::new(DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host()))
}

fn headers(pairs: &[(&str, &str)]) -> SimpleHeaders {
    let mut map = SimpleHeaders::new();
    for (key, value) in pairs {
        map.insert(SimpleHeader::custom(*key), vec![(*value).to_string()]);
    }
    map
}

/// Answer one request. A store refusal becomes its wire error response, so the
/// oracle sees every response shape.
async fn send(
    service: &DurableService<DurableStream>,
    method: SimpleMethod,
    name: &str,
    mut headers: SimpleHeaders,
    queries: Option<BTreeMap<String, String>>,
    body: &str,
) -> DurableResponse {
    if matches!(method, SimpleMethod::POST | SimpleMethod::PUT)
        && !headers
            .keys()
            .any(|key| key.to_string().eq_ignore_ascii_case("content-type"))
    {
        headers.insert(
            SimpleHeader::custom("content-type"),
            vec!["text/plain".to_string()],
        );
    }
    let request = DurableRequest {
        method,
        name: StreamName::parse(name).expect("valid stream name"),
        headers,
        queries,
        body: body.as_bytes().to_vec(),
    };
    match dispatch(service, &Ctx::host(), request).await {
        Ok(response) => response,
        Err(error) => error_response(&error),
    }
}

/// A lowercase hexadecimal view of raw body bytes.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(out, "{byte:02x}").expect("writing to a string cannot fail");
    }
    out
}

/// The normalized body shape.
fn body_shape(response: &DurableResponse) -> Value {
    if response.body.is_empty() {
        return json!({ "kind": "empty" });
    }
    let is_json = response
        .header("content-type")
        .is_some_and(|value| value.starts_with("application/json"));
    if is_json {
        let value: Value =
            serde_json::from_slice(&response.body).expect("a JSON body must parse as JSON");
        return json!({ "kind": "json", "value": value });
    }
    match std::str::from_utf8(&response.body) {
        Ok(text) => json!({ "kind": "text", "value": text }),
        Err(_) => json!({ "kind": "bytes", "value": hex(&response.body) }),
    }
}

/// The normalized view of one response, with the protocol headers only.
fn normalized(response: &DurableResponse) -> Value {
    json!({
        "status": response.status,
        "stream_next_offset": response.header(header::STREAM_NEXT_OFFSET),
        "stream_up_to_date": response.header(header::STREAM_UP_TO_DATE),
        "stream_closed": response.header(header::STREAM_CLOSED),
        "content_type": response.header("content-type"),
        "etag": response.header("etag").map(|_| "present"),
        "x_error_kind": response.header(header::ERROR_KIND),
        "body": body_shape(response),
    })
}

/// Compare one response with its fixture, or rewrite the fixture when
/// `UPDATE_GOLDEN` is set.
fn check(step: &str, response: &DurableResponse) {
    let actual = serde_json::to_string_pretty(&normalized(response)).expect("serialize");
    let path = golden_dir().join(format!("{step}.json"));

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(golden_dir()).expect("create the golden fixture directory");
        std::fs::write(&path, format!("{actual}\n")).expect("write the golden fixture");
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "missing golden fixture {}: {error}\nrun with UPDATE_GOLDEN=1 to write it",
            path.display()
        )
    });

    assert_eq!(
        actual.trim_end(),
        expected.trim_end(),
        "step `{step}` does not match its golden fixture {}",
        path.display()
    );
}

/// Steps 1 to 3: create the stream and append two chunks.
async fn create_and_append(service: &DurableService<DurableStream>) {
    check(
        "01-create",
        &send(
            service,
            SimpleMethod::PUT,
            "golden",
            headers(&[("content-type", "application/json")]),
            None,
            "",
        )
        .await,
    );

    check(
        "02-append-one",
        &send(
            service,
            SimpleMethod::POST,
            "golden",
            headers(&[("content-type", "application/json")]),
            None,
            r#"{"n":1}"#,
        )
        .await,
    );

    check(
        "03-append-two",
        &send(
            service,
            SimpleMethod::POST,
            "golden",
            headers(&[("content-type", "application/json")]),
            None,
            r#"{"n":2}"#,
        )
        .await,
    );
}

/// Steps 4 to 6: catch-up read, conditional read and head. Returns the tail
/// offset that the head reports.
async fn read_and_head(service: &DurableService<DurableStream>) -> String {
    let read = send(service, SimpleMethod::GET, "golden", headers(&[]), None, "").await;
    check("04-catch-up-read", &read);
    let tag = read
        .header("ETag")
        .expect("the read has an ETag")
        .to_string();

    check(
        "05-etag-read",
        &send(
            service,
            SimpleMethod::GET,
            "golden",
            headers(&[("if-none-match", &tag)]),
            None,
            "",
        )
        .await,
    );

    let head = send(
        service,
        SimpleMethod::HEAD,
        "golden",
        headers(&[]),
        None,
        "",
    )
    .await;
    check("06-head", &head);
    head.header(header::STREAM_NEXT_OFFSET)
        .expect("head reports the tail")
        .to_string()
}

/// Steps 7 and 8: close the stream, then refuse an append after the close.
async fn close_and_refuse_append(service: &DurableService<DurableStream>) {
    check(
        "07-close",
        &send(
            service,
            SimpleMethod::POST,
            "golden",
            headers(&[("stream-closed", "true")]),
            None,
            "",
        )
        .await,
    );

    check(
        "08-append-after-close",
        &send(
            service,
            SimpleMethod::POST,
            "golden",
            headers(&[("content-type", "application/json")]),
            None,
            r#"{"n":3}"#,
        )
        .await,
    );
}

/// Steps 9 to 11: a missing stream, a fork at the tail, then delete the fork.
async fn missing_fork_and_delete(service: &DurableService<DurableStream>, tail: &str) {
    check(
        "09-missing-stream",
        &send(service, SimpleMethod::GET, "absent", headers(&[]), None, "").await,
    );

    check(
        "10-fork",
        &send(
            service,
            SimpleMethod::PUT,
            "golden-fork",
            headers(&[
                ("content-type", "application/json"),
                ("stream-forked-from", "golden"),
                ("stream-fork-offset", tail),
            ]),
            None,
            "",
        )
        .await,
    );

    check(
        "11-delete",
        &send(
            service,
            SimpleMethod::DELETE,
            "golden-fork",
            headers(&[]),
            None,
            "",
        )
        .await,
    );
}

#[test]
fn golden_durable_http() {
    smol::block_on(async {
        let service = service();
        create_and_append(&service).await;
        let tail = read_and_head(&service).await;
        close_and_refuse_append(&service).await;
        missing_fork_and_delete(&service, &tail).await;
    });
}
