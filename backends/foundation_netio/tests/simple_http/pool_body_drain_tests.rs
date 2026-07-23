//! Pooled reuse after a response that carried a body.
//!
//! WHY: every pre-existing pool-reuse test uses a BODYLESS response — 204, HEAD,
//! or `Content-Length: 0` (see `pool_drain_tests.rs`). Reuse after a response
//! that actually carried bytes is therefore untested, and `drain_stream()` — the
//! thing that prepares a connection for reuse — has no idea how much is left to
//! read. It loops on `read()` until EOF or an error, which on a live keep-alive
//! connection means there is nothing to read and it waits out the read timeout.
//!
//! WHAT: three cases, separating correctness from cost —
//!   * body fully consumed by the caller, then reuse;
//!   * body NOT consumed by the caller, then reuse (drain has real work);
//!   * timing, to show whether returning a connection to the pool costs a read
//!     timeout of wall clock.
//!
//! HOW: one client, sequential requests to a keep-alive `TestHttpServer`, with
//! a request counter so a silent redial (which would make reuse tests pass for
//! the wrong reason) is visible as an extra connection rather than hidden.

use foundation_core::valtron::initialize_pool;
use foundation_netio::http::SimpleHttpClient;
use foundation_netio::shared::client::body_reader::collect_strings_from_send_safe;
use foundation_testing::http::{HttpResponse, TestHttpServer};
use serial_test::serial;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn body_response(payload: &[u8]) -> HttpResponse {
    HttpResponse {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "text/plain".to_string()),
            ("Content-Length".to_string(), payload.len().to_string()),
        ],
        body: payload.to_vec(),
    }
}

/// Server that counts requests and always answers with the same body.
fn counting_server(payload: &'static [u8]) -> (TestHttpServer, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&count);
    let server = TestHttpServer::with_response(move |_req| {
        seen.fetch_add(1, Ordering::SeqCst);
        body_response(payload)
    })
    .blocking_read(Some(Duration::from_secs(5)));
    (server, count)
}

#[test]
#[serial(valtron_pool)]
fn reuse_after_a_consumed_body_works() {
    let _guard = initialize_pool(45, None);
    let (server, count) = counting_server(b"hello world");
    let url = server.url("/body");
    let client = SimpleHttpClient::from_system();

    for which in ["first", "second", "third"] {
        let response = client
            .get(url.as_str())
            .unwrap()
            .build_client()
            .unwrap()
            .send()
            .unwrap_or_else(|e| panic!("{which} request failed: {e:?}"));
        assert_eq!(response.get_status().into_usize(), 200);

        // Consume the body, then let the response drop so the connection is
        // drained and pooled by `FinalizedResponse::drop`.
        let (_s, _h, body, _pool, _conn) = response.into_parts();
        let text = collect_strings_from_send_safe(body)
            .unwrap_or_else(|e| panic!("{which}: body read failed: {e:?}"));
        assert_eq!(text, "hello world", "{which}: wrong body");
    }

    assert_eq!(
        count.load(Ordering::SeqCst),
        3,
        "the server must have seen all three requests"
    );
}

#[test]
#[serial(valtron_pool)]
fn reuse_after_an_unconsumed_body_works() {
    // The caller reads only the status and drops the response. Everything the
    // server sent is still in the socket, so `drain_stream()` has real work to
    // do before the connection can be handed to the next request. If it gets
    // this wrong, the next response is parsed starting from the tail of the
    // previous body.
    let _guard = initialize_pool(45, None);
    let (server, count) = counting_server(b"unconsumed payload");
    let url = server.url("/body");
    let client = SimpleHttpClient::from_system();

    for which in ["first", "second", "third"] {
        let response = client
            .get(url.as_str())
            .unwrap()
            .build_client()
            .unwrap()
            .send()
            .unwrap_or_else(|e| {
                panic!(
                    "{which} request failed — reuse after an UNCONSUMED body is the \
                     case this test exists for: {e:?}"
                )
            });
        assert_eq!(
            response.get_status().into_usize(),
            200,
            "{which}: a status other than 200 suggests the response was parsed \
             from leftover bytes of the previous body"
        );
        drop(response);
    }

    assert_eq!(count.load(Ordering::SeqCst), 3);
}

#[test]
#[serial(valtron_pool)]
fn returning_a_connection_to_the_pool_is_not_charged_a_read_timeout() {
    // `drain_stream()` cannot know how much is left, so it reads until EOF or an
    // error. On a live keep-alive connection with nothing left to read, that
    // error is the READ TIMEOUT — meaning every pooled return would cost the
    // full timeout in wall clock. Pin that it does not.
    let _guard = initialize_pool(45, None);
    let (server, _count) = counting_server(b"x");
    let url = server.url("/body");

    // A deliberately long read timeout: if the drain is waiting one out, this
    // test takes ~10s per request and the assertion below fires.
    let client = SimpleHttpClient::from_system().read_timeout(Duration::from_secs(10));

    let started = Instant::now();
    for _ in 0..3 {
        let response = client
            .get(url.as_str())
            .unwrap()
            .build_client()
            .unwrap()
            .send()
            .expect("request should succeed");
        assert_eq!(response.get_status().into_usize(), 200);
        drop(response);
    }
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "three keep-alive requests took {elapsed:?}. With a 10s read timeout that \
         means returning a connection to the pool is waiting the timeout out — \
         drain_stream() reading until the socket errors, on a connection where \
         there is nothing left to read."
    );
}
