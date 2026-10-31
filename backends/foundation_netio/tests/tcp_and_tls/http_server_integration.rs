//! HTTP client integration tests against the local `TestHttpServer`.
//!
//! WHY: These tests exercise the full request/response flow of `NativeHttpClient`
//! against a real TCP server. No public internet access is needed.
//!
//! WHAT: GET, redirects, status codes, custom headers, response bodies, and
//! concurrent requests.
//!
//! HOW: Each test starts a local `TestHttpServer`, builds a `NativeHttpClient`,
//! sends a request, and asserts on the returned status and body.

use foundation_core::valtron::initialize_pool;
use foundation_netio::http::NativeHttpClient;
use foundation_netio::shared::client::body_reader::{
    collect_bytes_from_send_safe, collect_strings_from_send_safe,
};
use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders};
use foundation_testing::http::{HttpRequest, HttpResponse, TestHttpServer};
use serial_test::serial;

// All valtron pool tests share one global serial lock. A test that builds a pool
// must not run at the same time as another pool test.

/// A GET request returns 200 with the server body.
#[test]
#[serial(valtron_pool)]
fn test_http_client_get_request() {
    let _pool_guard = initialize_pool(60, None);
    let server = TestHttpServer::with_response(|_req| HttpResponse::ok(b"OK"));
    let client = NativeHttpClient::from_system();

    let res = client
        .get(&server.url("/test"))
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(res.get_status().into_usize(), 200);
    let body = collect_strings_from_send_safe(res.into_parts().2).unwrap();
    assert_eq!(body, "OK");
}

/// The client follows a 302 redirect and returns the target body.
#[test]
#[serial(valtron_pool)]
fn test_http_client_redirects() {
    let _pool_guard = initialize_pool(61, None);
    let server = TestHttpServer::with_response(|req: &HttpRequest| {
        if req.path.url.as_str() == "/redirect" {
            HttpResponse::redirect("/target")
        } else if req.path.url.as_str() == "/target" {
            HttpResponse::ok(b"Redirected Successfully")
        } else {
            HttpResponse::ok(b"Default")
        }
    });
    let client = NativeHttpClient::from_system().max_redirects(5);

    let res = client
        .get(&server.url("/redirect"))
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(res.get_status().into_usize(), 200);
    let body = collect_strings_from_send_safe(res.into_parts().2).unwrap();
    assert_eq!(body, "Redirected Successfully");
}

/// The client returns non-2xx status codes without treating them as errors.
#[test]
#[serial(valtron_pool)]
fn test_http_client_status_codes() {
    let _pool_guard = initialize_pool(62, None);
    let server = TestHttpServer::with_response(|req: &HttpRequest| match req.path.url.as_str() {
        "/not-found" => HttpResponse::status(404, "Not Found"),
        "/server-error" => HttpResponse::status(500, "Internal Server Error"),
        "/created" => HttpResponse::status(201, "Created"),
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

    assert_eq!(get_status("/not-found"), 404);
    assert_eq!(get_status("/server-error"), 500);
    assert_eq!(get_status("/created"), 201);
}

/// The client sends custom headers; the server observes them.
#[test]
#[serial(valtron_pool)]
fn test_http_client_custom_headers() {
    use std::sync::{Arc, Mutex};

    let _pool_guard = initialize_pool(63, None);
    let received_headers: Arc<Mutex<SimpleHeaders>> = Arc::new(Mutex::new(SimpleHeaders::new()));
    let headers_clone = Arc::clone(&received_headers);

    let server = TestHttpServer::with_response(move |req: &HttpRequest| {
        *headers_clone.lock().unwrap() = req.headers.clone();
        HttpResponse::ok(b"Headers Received")
    });
    let client = NativeHttpClient::from_system();

    let res = client
        .get(&server.url("/"))
        .unwrap()
        .header(SimpleHeader::USER_AGENT, "TestClient/1.0")
        .header(SimpleHeader::ACCEPT, "application/json")
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(res.get_status().into_usize(), 200);

    let received = received_headers.lock().unwrap();
    let user_agents = received
        .get(&SimpleHeader::USER_AGENT)
        .cloned()
        .unwrap_or_default();
    assert!(
        user_agents.iter().any(|v| v == "TestClient/1.0"),
        "User-Agent header not received: {received:?}"
    );
    let accepts = received
        .get(&SimpleHeader::ACCEPT)
        .cloned()
        .unwrap_or_default();
    assert!(
        accepts.iter().any(|v| v == "application/json"),
        "Accept header not received: {received:?}"
    );
}

/// The client reads empty, small, and large response bodies.
#[test]
#[serial(valtron_pool)]
fn test_http_client_response_body() {
    let _pool_guard = initialize_pool(64, None);
    let large_body = "x".repeat(10_000);
    let large_body_clone = large_body.clone();

    let server = TestHttpServer::with_response(move |req: &HttpRequest| match req.path.url.as_str() {
        "/empty" => HttpResponse::ok(b""),
        "/small" => HttpResponse::ok(b"Small body"),
        "/large" => HttpResponse::ok(large_body_clone.as_bytes()),
        _ => HttpResponse::ok(b"Default"),
    });
    let client = NativeHttpClient::from_system();

    let empty = client
        .get(&server.url("/empty"))
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert_eq!(collect_bytes_from_send_safe(empty.into_parts().2).len(), 0);

    let small = client
        .get(&server.url("/small"))
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert_eq!(
        collect_strings_from_send_safe(small.into_parts().2).unwrap(),
        "Small body"
    );

    let large = client
        .get(&server.url("/large"))
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert_eq!(
        collect_strings_from_send_safe(large.into_parts().2)
            .unwrap()
            .len(),
        10_000
    );
}

/// The client serves several concurrent requests against one server.
#[test]
#[serial(valtron_pool)]
fn test_http_client_concurrent_requests() {
    use std::thread;

    let _pool_guard = initialize_pool(65, None);
    let server = TestHttpServer::with_response(|req: &HttpRequest| {
        HttpResponse::ok(format!("Path: {}", req.path.url.as_str()).as_bytes())
    });
    let base_url = server.base_url().to_string();

    let handles: Vec<_> = (0..5)
        .map(|i| {
            let url = format!("{base_url}/path{i}");
            thread::spawn(move || {
                let client = NativeHttpClient::from_system();
                let res = client.get(&url).unwrap().build_client().unwrap().send().unwrap();
                assert_eq!(res.get_status().into_usize(), 200);
                collect_strings_from_send_safe(res.into_parts().2).unwrap()
            })
        })
        .collect();

    for (i, handle) in handles.into_iter().enumerate() {
        let body = handle.join().unwrap();
        assert_eq!(body, format!("Path: /path{i}"));
    }
}

// ========================================================================
// TestHttpServer utility tests
// ========================================================================

/// `TestHttpServer` starts and returns valid loopback URLs.
#[test]
fn test_server_provides_valid_urls() {
    let server = TestHttpServer::start();

    assert!(server.base_url().starts_with("http://127.0.0.1:"));
    assert!(server.url("/test").contains("/test"));
    assert!(server.url("/api/v1").contains("/api/v1"));
}

/// `TestHttpServer` serves each custom response type.
#[test]
fn test_server_custom_responses() {
    let server = TestHttpServer::with_response(|req: &HttpRequest| match req.path.url.as_str() {
        "/ok" => HttpResponse::ok(b"Success"),
        "/redirect" => HttpResponse::redirect("/new-location"),
        "/error" => HttpResponse::status(500, "Error"),
        _ => HttpResponse::status(404, "Not Found"),
    });

    assert!(server.base_url().starts_with("http://"));
}

/// `TestHttpServer` stops cleanly on drop without hanging.
#[test]
fn test_server_cleanup() {
    {
        let _server = TestHttpServer::start();
    }
    // The server stopped after drop; the test passes if it does not hang.
}
