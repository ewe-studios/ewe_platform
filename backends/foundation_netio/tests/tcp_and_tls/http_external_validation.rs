//! HTTP client validation tests against a local server that mimics httpbin.
//!
//! WHY: The original tests hit httpbin.org over the public internet. They now
//! run against a local server, so CI needs no network access.
//!
//! WHAT: GET echo, redirects, status codes, header echo, and HTTPS.
//!
//! HOW: A local `TestHttpServer` mimics the httpbin endpoints. The HTTPS test
//! uses `TestHttpsServer` and a client that trusts the test certificate.

use std::net::SocketAddr;

use foundation_core::valtron::initialize_pool;
use foundation_netio::http::NativeHttpClient;
use foundation_netio::shared::client::body_reader::collect_strings_from_send_safe;
use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::{HttpClientBuilder, PreparedRequestBuilder};
use foundation_testing::http::{
    test_tls_connector, HttpRequest, HttpResponse, TestHttpServer, TestHttpsServer,
};
use serial_test::serial;

// All valtron pool tests share one global serial lock. A test that builds a pool
// must not run at the same time as another pool test.

/// A GET against the local echo server returns the requested path in the body.
#[test]
#[serial(valtron_pool)]
fn test_local_get_echoes_path() {
    let _pool_guard = initialize_pool(70, None);
    let server = TestHttpServer::with_response(|req: &HttpRequest| {
        HttpResponse::ok(format!(r#"{{"url": "{}"}}"#, req.path.url).as_bytes())
    });
    let client = NativeHttpClient::from_system();

    let res = client
        .get(&server.url("/get"))
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(res.get_status().into_usize(), 200);
    let body = collect_strings_from_send_safe(res.into_parts().2).unwrap();
    assert!(body.contains("\"url\": \"/get\""), "body: {body}");
}

/// A single redirect (httpbin `/redirect/1`) resolves to the target body.
#[test]
#[serial(valtron_pool)]
fn test_local_redirect() {
    let _pool_guard = initialize_pool(71, None);
    let server = TestHttpServer::with_response(|req: &HttpRequest| {
        if req.path.url == "/redirect/1" {
            HttpResponse::redirect("/get")
        } else {
            HttpResponse::ok(b"Redirected")
        }
    });
    let client = NativeHttpClient::from_system().max_redirects(5);

    let res = client
        .get(&server.url("/redirect/1"))
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(res.get_status().into_usize(), 200);
    let body = collect_strings_from_send_safe(res.into_parts().2).unwrap();
    assert_eq!(body, "Redirected");
}

/// The client completes an HTTPS request against a local TLS server.
#[test]
#[serial(valtron_pool)]
fn test_local_https_get() {
    let _pool_guard = initialize_pool(72, Some(4));
    let server = TestHttpsServer::with_response(|_req| HttpResponse::ok(b"hello from TLS"));

    // `localhost` is the name on the test certificate; it resolves to the
    // server's socket.
    let client = HttpClientBuilder::new()
        .resolve_to(SocketAddr::from(([127, 0, 0, 1], server.port())))
        .with_tls_connector(test_tls_connector())
        .build();

    let request = PreparedRequestBuilder::get("https://localhost/get")
        .unwrap()
        .build();
    let response = client
        .send(request)
        .expect("HTTPS request should succeed");

    assert_eq!(response.get_status().into_usize(), 200);
}

/// The client returns the status codes from the `/status/N` endpoints.
#[test]
#[serial(valtron_pool)]
fn test_local_status_codes() {
    let _pool_guard = initialize_pool(73, None);
    let server = TestHttpServer::with_response(|req: &HttpRequest| match req.path.url.as_str() {
        "/status/404" => HttpResponse::status(404, "Not Found"),
        "/status/500" => HttpResponse::status(500, "Server Error"),
        "/status/201" => HttpResponse::status(201, "Created"),
        _ => HttpResponse::ok(b"OK"),
    });
    let client = NativeHttpClient::from_system();

    let get_status = |path: &str| {
        client
            .get(&server.url(path))
            .unwrap()
            .build_client()
            .unwrap()
            .send()
            .unwrap()
            .get_status()
            .into_usize()
    };

    assert_eq!(get_status("/status/404"), 404);
    assert_eq!(get_status("/status/500"), 500);
    assert_eq!(get_status("/status/201"), 201);
}

/// The `/headers` endpoint echoes the request header values back in the body.
#[test]
#[serial(valtron_pool)]
fn test_local_header_echo() {
    let _pool_guard = initialize_pool(74, None);
    let server = TestHttpServer::with_response(|req: &HttpRequest| {
        let mut echoed = String::new();
        for values in req.headers.values() {
            for value in values {
                echoed.push_str(value);
                echoed.push('\n');
            }
        }
        HttpResponse::ok(echoed.as_bytes())
    });
    let client = NativeHttpClient::from_system();

    let res = client
        .get(&server.url("/headers"))
        .unwrap()
        .header(SimpleHeader::USER_AGENT, "foundation-http/1.0")
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(res.get_status().into_usize(), 200);
    let body = collect_strings_from_send_safe(res.into_parts().2).unwrap();
    assert!(
        body.contains("foundation-http/1.0"),
        "header value not echoed: {body}"
    );
}
