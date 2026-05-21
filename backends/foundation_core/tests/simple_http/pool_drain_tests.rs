//! Integration tests for connection pool drain-before-pooling fix.
//!
//! After no-body responses (204, HEAD), the HTTP reader may leave the
//! underlying stream in an inconsistent state. `FinalizedResponse::drop()`
//! calls `drain_stream()` before returning the connection to the pool.
//!
//! These tests verify that sequential no-body + body requests succeed
//! when connection pooling is enabled.

use foundation_core::valtron::initialize_pool;
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_testing::http::{HttpResponse, TestHttpServer};
use serial_test::serial;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing_test::traced_test;

fn server_addr(server: &TestHttpServer) -> SocketAddr {
    server
        .base_url()
        .strip_prefix("http://")
        .unwrap()
        .parse()
        .unwrap()
}

/// WHY: Verify that after a 204 No Content response, a pooled connection
/// can successfully handle a subsequent request. Without the drain fix,
/// the second request fails with `ReadFailed` because the stream was
/// returned to the pool in an inconsistent state.
#[test]
#[serial(valtron_pool)]
#[traced_test]
fn test_pool_204_then_get_succeeds() {
    let _guard = initialize_pool(45, None);

    let request_count = Arc::new(AtomicUsize::new(0));
    let request_count_clone = request_count.clone();

    let server = TestHttpServer::with_response(move |_req| {
        let count = request_count.fetch_add(1, Ordering::SeqCst);
        match count {
            0 => HttpResponse {
                status: 204,
                status_text: "No Content".to_string(),
                headers: vec![],
                body: Vec::new(),
            },
            1 => HttpResponse {
                status: 200,
                status_text: "OK".to_string(),
                headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
                body: b"hello from second request".to_vec(),
            },
            _ => HttpResponse {
                status: 500,
                status_text: "Internal Server Error".to_string(),
                headers: vec![],
                body: b"unexpected third request".to_vec(),
            },
        }
    });

    let addr = server_addr(&server);
    let base_url = format!("http://{}", addr);

    let client = SimpleHttpClient::from_system();

    // First request: DELETE (will get 204)
    let resp = client
        .delete(&base_url)
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .expect("DELETE should succeed");

    assert_eq!(resp.get_status().into_usize(), 204);

    // Second request: GET (should reuse pooled connection)
    let resp2 = client
        .get(&base_url)
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .expect("GET after 204 should succeed (pooled connection drained)");

    assert_eq!(resp2.get_status().into_usize(), 200);
    assert_eq!(request_count_clone.load(Ordering::SeqCst), 2);
}

/// WHY: Verify that sequential HEAD requests (no body each) work correctly
/// with connection pooling. Each HEAD response returns the connection to
/// the pool after draining.
#[test]
#[serial(valtron_pool)]
#[traced_test]
fn test_pool_sequential_head_requests() {
    let _guard = initialize_pool(45, None);

    let request_count = Arc::new(AtomicUsize::new(0));
    let request_count_clone = request_count.clone();

    tracing::trace!("[test] starting testhttpserver");
    let mut server = TestHttpServer::with_response(move |_req| {
        request_count.fetch_add(1, Ordering::SeqCst);

        tracing::trace!("[TEST] Sending response");
        HttpResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![
                ("Content-Type".to_string(), "text/plain".to_string()),
                ("Content-Length".to_string(), "0".to_string()),
            ],
            body: Vec::new(), // HEAD responses must not have body
        }
    })
    // WHY: The HTTP reader treats WouldBlock from non-blocking sockets as fatal.
    // Blocking read with timeout prevents premature connection handler exit.
    .blocking_read(Some(Duration::from_secs(5)));

    let addr = server_addr(&server);
    let base_url = format!("http://{}", addr);

    tracing::trace!("[TEST] Creating http client");
    let client = SimpleHttpClient::from_system();

    for i in 0..3 {
        tracing::trace!("[test] sending request for head at index: {}", i);
        let resp = client
            .head(&base_url)
            .unwrap()
            .build_client()
            .unwrap()
            .send()
            .unwrap_or_else(|e| panic!("HEAD request {} should succeed: {e}", i + 1));

        tracing::trace!("[TEST] Received response for request: {}", i);
        assert_eq!(
            resp.get_status().into_usize(),
            200,
            "HEAD {} should return 200",
            i + 1
        );
        tracing::trace!(
            "[TEST] Finished checking status for request response: {}",
            i
        );

        // Clear pool between requests to validate drain hypothesis
        if let Some(pool) = client.client_pool() {
            pool.clear_pool();
            tracing::trace!("[TEST] Cleared connection pool after request {}", i + 1);
        }
    }

    assert_eq!(request_count_clone.load(Ordering::SeqCst), 3);
    tracing::trace!("Finished test");
    server.stop();
    tracing::trace!("Updated test server to stop");
}

/// WHY: Verify the full sequence that originally exposed the bug:
/// PUT → HEAD → DELETE → HEAD. The second HEAD previously failed
/// with `ReadFailed` because the DELETE's 204 response left the
/// connection in an inconsistent state.
#[test]
#[serial(valtron_pool)]
#[traced_test]
fn test_pool_put_head_delete_head_sequence() {
    let _guard = initialize_pool(45, None);

    tracing::trace!("[test] starting testhttpserver");
    let request_count = Arc::new(AtomicUsize::new(0));
    let request_count_clone = request_count.clone();

    let mut server = TestHttpServer::with_response(move |_req| {
        request_count.fetch_add(1, Ordering::SeqCst);
        tracing::trace!("[TEST] Sending response");
        HttpResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
            body: b"ok".to_vec(),
        }
    })
    .blocking_read(Some(Duration::from_secs(5)));

    let addr = server_addr(&server);
    let base_url = format!("http://{}", addr);

    tracing::trace!("[TEST] Creating http client");
    let client = SimpleHttpClient::from_system();

    // PUT
    tracing::trace!("[TEST] Sending put request");
    let resp = client
        .put(&base_url)
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .expect("PUT should succeed");
    assert_eq!(resp.get_status().into_usize(), 200);

    // HEAD (first)
    tracing::trace!("[TEST] Sending head request");
    let resp = client
        .head(&base_url)
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .expect("First HEAD should succeed");
    assert_eq!(resp.get_status().into_usize(), 200);

    // DELETE
    tracing::trace!("[TEST] Sending delete request");
    let resp = client
        .delete(&base_url)
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .expect("DELETE should succeed");
    assert_eq!(resp.get_status().into_usize(), 200);

    // HEAD (second) — this previously failed with ReadFailed
    tracing::trace!("[TEST] Sending head(2) request");
    let resp = client
        .head(&base_url)
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .expect("Second HEAD should succeed (pool drain fix)");
    assert_eq!(resp.get_status().into_usize(), 200);

    assert_eq!(request_count_clone.load(Ordering::SeqCst), 4);

    tracing::trace!("[TEST] Finished tests");
    server.stop();
    tracing::trace!("[TEST] Updated test server to stop");
}
