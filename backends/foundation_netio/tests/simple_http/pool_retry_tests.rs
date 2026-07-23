//! The client survives a pooled connection the peer closed while it was idle.
//!
//! WHY: `ConnectionPool::checkout` validates only idle time — it cannot know
//! whether the peer has since closed the socket, and a liveness probe would not
//! help because the peer can close between the probe and the write. So a dead
//! pooled connection is not preventable; it has to be *recovered from*. Before
//! the retry existed, the second request over a silently-closed keep-alive
//! connection failed with a bare `WriteFailed` for a request that never reached
//! the server at all.
//!
//! WHAT: drives two sequential requests through one client against a server
//! that closes the TCP connection after each response *without* announcing it
//! with `Connection: close`. That is the exact shape the retry exists for: the
//! client has no way to know the pooled socket is dead until it writes to it.
//!
//! HOW: `TestHttpServer::close_after_response(true)` closes the connection but
//! adds no header, so the client pools the (now dead) socket after request one.
//! Request two checks it out, the write fails, and the retry redials. A server
//! that announced `Connection: close` would exercise a different path — the
//! connection would never be pooled — so it would not test this at all.

use foundation_netio::http::SimpleHttpClient;
use foundation_testing::http::HttpResponse;

/// A bodyless 200.
///
/// Deliberately `Content-Length: 0` rather than a response with a body: an
/// unconsumed body makes `FinalizedResponse::drop`'s `drain_stream()` the thing
/// under test (it reads until the socket errors, which on a live keep-alive
/// connection means waiting out the read timeout). This test is about the
/// closed-connection retry, so keep the body out of it.
fn empty_ok() -> HttpResponse {
    HttpResponse {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![("Content-Length".to_string(), "0".to_string())],
        body: Vec::new(),
    }
}
use foundation_testing::TestHttpServer;
use serial_test::serial;

/// Issue one GET and return its status code.
///
/// The response is deliberately allowed to drop normally rather than being taken
/// apart with `into_parts()`: `FinalizedResponse::drop` is what drains the body
/// and returns the connection to the pool. Consuming the parts here would strand
/// the connection, every request would dial fresh, and this test would pass
/// whether or not the retry exists — which is exactly what it did at first.
fn get_status(client: &SimpleHttpClient, url: &str, which: &str) -> usize {
    let response = client
        .get(url)
        .unwrap_or_else(|e| panic!("{which}: building the request failed: {e:?}"))
        .build_client()
        .unwrap_or_else(|e| panic!("{which}: building the client failed: {e:?}"))
        .send()
        .unwrap_or_else(|e| {
            panic!(
                "{which}: request failed. A failure here on the SECOND request is the \
                 regression this test exists for — the pooled connection was closed by \
                 the peer and the write was not retried on a fresh one: {e:?}"
            )
        });

    response.get_status().into_usize()
}

#[test]
#[serial(valtron_pool)]
fn a_second_request_survives_a_pooled_connection_the_peer_closed() {
    let _pool_guard = foundation_core::valtron::initialize_pool(42, None);

    // Closes after every response, but never says so in a header — so the
    // client has every reason to believe the connection is reusable.
    let server = TestHttpServer::with_response(|_req| empty_ok())
        .close_after_response(true)
        .blocking_read(Some(std::time::Duration::from_secs(5)));
    let url = server.url("/ping");

    let client = SimpleHttpClient::from_system();

    // First request: dials fresh, succeeds, and the socket goes into the pool.
    assert_eq!(get_status(&client, url.as_str(), "first request"), 200);

    // Second request: checks that dead socket back out. The write fails, and the
    // retry has to redial transparently.
    assert_eq!(get_status(&client, url.as_str(), "second request"), 200);

    // Third, to show the recovery is repeatable rather than a one-off that
    // happens to leave the pool in a good state.
    assert_eq!(get_status(&client, url.as_str(), "third request"), 200);
}

#[test]
#[serial(valtron_pool)]
fn a_keep_alive_server_still_reuses_connections() {
    // The retry must not turn into "always redial". A server that honours
    // keep-alive should still see its connection reused — this test would still
    // pass if reuse broke, but it pins that the retry path does not error on a
    // perfectly healthy pooled connection.
    let _pool_guard = foundation_core::valtron::initialize_pool(42, None);

    let server = TestHttpServer::with_response(|_req| empty_ok())
        .blocking_read(Some(std::time::Duration::from_secs(5)));
    let url = server.url("/alive");

    let client = SimpleHttpClient::from_system();
    for which in ["first", "second", "third"] {
        assert_eq!(get_status(&client, url.as_str(), which), 200);
    }
}
