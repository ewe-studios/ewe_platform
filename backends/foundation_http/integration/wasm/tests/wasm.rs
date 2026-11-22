//! The twenty contract cases of spec 88 feature 02, on the wasm-bindgen harness.
//!
//! WHY: the wasm bridge, the Cloudflare bridge, the serve-writer path, the
//! response builders and the wasm streams are compiled by no run today (#570).
//! WHAT: cases 1 to 20 of `features/02-http-runtime/feature.md` section 6.
//! HOW: every case builds its own app and sends a real `web_sys::Request`. The
//! cases dispatch in memory; none reaches the network. The bridge, the router
//! and the handlers are the public path (rule R13).

#![cfg(target_arch = "wasm32")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use foundation_core::valtron::SimulatedClock;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::shared::middleware::{MiddlewareResult, RequestMiddleware};
use foundation_http::shared::serve::{ConnectionResult, ServeWriter};
use foundation_http::wasm::cf_conn::CfConn;
use foundation_http::wasm::response::{WasmResponse, from_wasm};
use foundation_http::wasm::serve_writer::{
    handle_request as handle_writer_request, run_middleware_writer,
};
use foundation_http::wasm::stream::WasmStream;
use foundation_http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};
use foundation_http_wasm_tests::{
    CfFnHandler, EchoBodyHandler, FnHandler, ORIGIN, body_text, cf_app_with, header, header_values,
    host_name, request, request_db, test_env, web_app_with, web_app_with_ctx,
};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test;

const GET: SimpleMethod = SimpleMethod::GET;
const POST: SimpleMethod = SimpleMethod::POST;

// ── Shared handlers and middleware ──────────────────────────────────

/// Middleware that answers 401 and stops the chain (cases 6 and 7).
struct RejectMiddleware;

impl RequestMiddleware for RejectMiddleware {
    fn handle(&self, _ctx: &Ctx, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        MiddlewareResult::Response(SimpleOutgoingResponse {
            proto: Proto::HTTP11,
            status: Status::Unauthorized,
            headers: SimpleHeaders::new(),
            body: Some(SendSafeBody::Text("Unauthorized".into())),
            trailers: SimpleHeaders::new(),
        })
    }
}

/// Middleware that appends its label and continues (case 7).
struct RecordingMiddleware {
    label: u8,
    order: Arc<Mutex<Vec<u8>>>,
}

impl RequestMiddleware for RecordingMiddleware {
    fn handle(&self, _ctx: &Ctx, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        self.order.lock().expect("the order lock").push(self.label);
        MiddlewareResult::Continue
    }
}

/// Middleware that adds a request header and continues (case 17).
struct AddRequestHeader;

impl RequestMiddleware for AddRequestHeader {
    fn handle(&self, _ctx: &Ctx, req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        req.headers
            .entry(SimpleHeader::from("X-Middleware".to_string()))
            .or_default()
            .push("passed".to_string());
        MiddlewareResult::Continue
    }
}

/// A serve-writer handler that renders one plain-text response (cases 16, 18).
struct TextWriterHandler {
    body: &'static str,
}

impl ServeWriter for TextWriterHandler {
    fn serve_writer(
        &self,
        _ctx: &Ctx,
        _req: SimpleIncomingRequest,
        conn: &mut dyn std::io::Write,
    ) -> ConnectionResult {
        // `respond_to_buffer` takes a sized writer; `&mut dyn Write` is one.
        let mut writer = conn;
        foundation_http::shared::serve::respond_to_buffer::text(&mut writer, 200, self.body)
            .expect("render the response to the in-memory stream");
        ConnectionResult::Keep
    }
}

/// A marker a handler stores in the request context (case 10).
struct RequestMarker;

/// Build a `SimpleIncomingRequest` for the in-memory writer path.
fn writer_request(method: SimpleMethod, url: &str, body: SendSafeBody) -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_parsed_url(url)
        .with_method(method)
        .with_proto(Proto::HTTP11)
        .with_headers(SimpleHeaders::new())
        .with_body(body)
        .build()
        .expect("a valid request")
}

// ── T2: the web bridge (cases 1 to 11) ──────────────────────────────

/// Case 1. Oracle: example. Status, header and body round trip.
#[wasm_bindgen_test]
async fn get_route_returns_status_headers_and_body() {
    let app = web_app_with(|app| {
        app.router.add_route_web(
            GET,
            "/hello",
            FnHandler::new(|_ctx, _req, conn| {
                conn.set_status(200);
                conn.set_header("content-type", "text/plain");
                conn.set_header("x-case", "one");
                conn.set_body(b"hi".to_vec());
            }),
        );
    });

    let response = app
        .handle_request(request("GET", &format!("{ORIGIN}/hello"), &[], None))
        .await
        .expect("the GET dispatches");
    assert_eq!(response.status(), 200, "{}: case 1: status", host_name());
    assert_eq!(
        header(&response, "x-case").as_deref(),
        Some("one"),
        "{}: case 1: header",
        host_name()
    );
    assert_eq!(
        body_text(&response).await,
        "hi",
        "{}: case 1: body",
        host_name()
    );
}

/// Case 2. Oracle: example. The handler reads the body text and its length.
#[wasm_bindgen_test]
async fn post_body_reaches_the_handler() {
    let app = web_app_with(|app| {
        app.router
            .add_route_web(POST, "/echo", Arc::new(EchoBodyHandler));
    });

    let response = app
        .handle_request(request(
            "POST",
            &format!("{ORIGIN}/echo"),
            &[("x-probe", "two")],
            Some(b"hello"),
        ))
        .await
        .expect("the POST dispatches");
    assert_eq!(response.status(), 200, "{}: case 2: status", host_name());
    assert_eq!(
        header(&response, "x-body-len").as_deref(),
        Some("5"),
        "{}: case 2: the handler read five bytes",
        host_name()
    );
    assert_eq!(
        body_text(&response).await,
        "5:hello",
        "{}: case 2: body",
        host_name()
    );
}

/// Case 3. Oracle: expected outcome. A miss is a 404 response, not a throw.
#[wasm_bindgen_test]
async fn unknown_path_returns_404() {
    let app = web_app_with(|app| {
        app.router.add_route_web(
            GET,
            "/hello",
            FnHandler::new(|_ctx, _req, conn| conn.set_body(b"hi".to_vec())),
        );
    });

    let response = app
        .handle_request(request("GET", &format!("{ORIGIN}/missing"), &[], None))
        .await
        .expect("a route miss answers, it does not throw");
    assert_eq!(response.status(), 404, "{}: case 3: status", host_name());
}

/// Case 4. Oracle: expected outcome. A known path with the wrong method is 405.
#[wasm_bindgen_test]
async fn wrong_method_returns_405() {
    let app = web_app_with(|app| {
        app.router.add_route_web(
            GET,
            "/hello",
            FnHandler::new(|_ctx, _req, conn| conn.set_body(b"hi".to_vec())),
        );
    });

    let response = app
        .handle_request(request("POST", &format!("{ORIGIN}/hello"), &[], Some(b"x")))
        .await
        .expect("a method miss answers, it does not throw");
    assert_eq!(response.status(), 405, "{}: case 4: status", host_name());
}

/// Case 5. Oracle: example. `%2F` arrives as `/`, so the route sees two
/// decoded segment parameters and the handler reads the decoded path.
#[wasm_bindgen_test]
async fn path_parameter_is_decoded() {
    let app = web_app_with(|app| {
        app.router.add_route_web(
            GET,
            "/items/:kind/:name",
            FnHandler::new(|_ctx, req, conn| {
                conn.set_status(200);
                conn.set_body(req.request_uri.path().as_bytes().to_vec());
            }),
        );
    });

    let response = app
        .handle_request(request("GET", &format!("{ORIGIN}/items/a%2Fb"), &[], None))
        .await
        .expect("the request dispatches");
    assert_eq!(
        response.status(),
        200,
        "{}: case 5: the decoded path matches the two parameters",
        host_name()
    );
    assert_eq!(
        body_text(&response).await,
        "/items/a/b",
        "{}: case 5: the handler reads the decoded path",
        host_name()
    );
}

/// Case 6. Oracle: expected outcome plus a counter. The handler never runs.
#[wasm_bindgen_test]
async fn middleware_short_circuit_returns_401() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_handler = Arc::clone(&calls);
    let app = web_app_with(|app| {
        app.middleware(RejectMiddleware);
        app.router.add_route_web(
            GET,
            "/hello",
            FnHandler::new(move |_ctx, _req, conn| {
                calls_for_handler.fetch_add(1, Ordering::SeqCst);
                conn.set_body(b"handler ran".to_vec());
            }),
        );
    });

    let response = app
        .handle_request(request("GET", &format!("{ORIGIN}/hello"), &[], None))
        .await
        .expect("the request dispatches");
    assert_eq!(response.status(), 401, "{}: case 6: status", host_name());
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "{}: case 6: the handler must not run",
        host_name()
    );
}

/// Case 7. Oracle: model. The recorded order equals the registration order.
#[wasm_bindgen_test]
async fn middleware_runs_in_registration_order() {
    let order = Arc::new(Mutex::new(Vec::new()));
    let app = web_app_with(|app| {
        for label in [b'a', b'b', b'c'] {
            app.middleware(RecordingMiddleware {
                label,
                order: Arc::clone(&order),
            });
        }
        app.router.add_route_web(
            GET,
            "/hello",
            FnHandler::new(|_ctx, _req, conn| conn.set_body(b"hi".to_vec())),
        );
    });

    let response = app
        .handle_request(request("GET", &format!("{ORIGIN}/hello"), &[], None))
        .await
        .expect("the request dispatches");
    assert_eq!(response.status(), 200, "{}: case 7: status", host_name());
    assert_eq!(
        *order.lock().expect("the order lock"),
        vec![b'a', b'b', b'c'],
        "{}: case 7: middleware order",
        host_name()
    );
}

/// Case 8. Oracle: example. Duplicate response headers keep append order.
///
/// The case uses a normal header, not `Set-Cookie`: Chromium hides
/// `Set-Cookie` from a `Response` that a page constructs, so its order cannot
/// be read there. The Fetch API joins the duplicate values of any other header
/// in append order, which is the property under test.
#[wasm_bindgen_test]
async fn duplicate_response_headers_keep_their_order() {
    let app = web_app_with(|app| {
        app.router.add_route_web(
            GET,
            "/headers",
            FnHandler::new(|_ctx, _req, conn| {
                conn.set_status(200);
                conn.append_header("x-order", "a=1");
                conn.append_header("x-order", "b=2");
            }),
        );
    });

    let response = app
        .handle_request(request("GET", &format!("{ORIGIN}/headers"), &[], None))
        .await
        .expect("the request dispatches");
    let values = header_values(&response, "x-order");
    assert_eq!(
        values,
        vec!["a=1".to_string(), "b=2".to_string()],
        "{}: case 8: duplicate header order",
        host_name()
    );
}

/// Case 9. Oracle: fault. A panicking handler answers 500 and the host stays up.
///
/// Gated: on `wasm32-unknown-unknown` a Rust panic aborts the module, so the
/// case traps the host before it can assert. Issue #742 tracks the fix.
#[cfg(feature = "panicking-handler-case")]
#[wasm_bindgen_test]
async fn handler_panic_becomes_a_500() {
    let app = web_app_with(|app| {
        app.router.add_route_web(
            GET,
            "/panic",
            FnHandler::new(|_ctx, _req, _conn| panic!("the handler panics")),
        );
    });

    let response = app
        .handle_request(request("GET", &format!("{ORIGIN}/panic"), &[], None))
        .await
        .expect("the bridge turns the panic into a response");
    assert_eq!(response.status(), 500, "{}: case 9: status", host_name());
}

/// Case 10. Oracle: expected outcome. An insert in request A is absent in B.
#[wasm_bindgen_test]
async fn request_fork_keeps_ctx_inserts_private() {
    let app = web_app_with(|app| {
        app.router.add_route_web(
            GET,
            "/marker",
            FnHandler::new(|ctx, _req, conn| {
                let seen = ctx.get::<RequestMarker>().is_some();
                if !seen {
                    ctx.store(RequestMarker);
                }
                conn.set_status(200);
                conn.set_body(if seen {
                    b"present".to_vec()
                } else {
                    b"absent".to_vec()
                });
            }),
        );
    });

    let first = app
        .handle_request(request("GET", &format!("{ORIGIN}/marker"), &[], None))
        .await
        .expect("the first request dispatches");
    let second = app
        .handle_request(request("GET", &format!("{ORIGIN}/marker"), &[], None))
        .await
        .expect("the second request dispatches");

    assert_eq!(
        body_text(&first).await,
        "absent",
        "{}: case 10: the first request inserts",
        host_name()
    );
    assert_eq!(
        body_text(&second).await,
        "absent",
        "{}: case 10: the second request must not see the first insert",
        host_name()
    );
}

/// Case 11. Oracle: expected outcome. The handler reads the injected clock.
#[wasm_bindgen_test]
async fn injected_clock_reaches_the_handler() {
    // The simulated clock starts at zero and moves 1234 s by hand. A host clock
    // cannot report an elapsed time that large this early.
    let clock = Arc::new(SimulatedClock::new(std::time::Duration::ZERO));
    clock.advance(std::time::Duration::from_secs(1234));

    let app = web_app_with_ctx(Ctx::fixed(clock), |app| {
        app.router.add_route_web(
            GET,
            "/clock",
            FnHandler::new(|ctx, _req, conn| {
                let elapsed = ctx.clock().elapsed().as_secs();
                conn.set_status(200);
                conn.set_body(elapsed.to_string().into_bytes());
            }),
        );
    });

    let response = app
        .handle_request(request("GET", &format!("{ORIGIN}/clock"), &[], None))
        .await
        .expect("the request dispatches");
    assert_eq!(
        body_text(&response).await,
        "1234",
        "{}: case 11: the handler must read the injected clock",
        host_name()
    );
}

// ── T3: the Cloudflare bridge (cases 12 to 15) ──────────────────────

/// Case 12. Oracle: example. A route answers with `env` undefined.
#[wasm_bindgen_test]
async fn cf_dispatch_routes_and_answers() {
    let app = cf_app_with(|app| {
        app.router.add_route_cf(
            GET,
            "/cf",
            CfFnHandler::new(|_ctx, _req, conn| {
                conn.set_status(200);
                conn.set_header("x-cf", "yes");
                conn.set_body(b"cf-ok".to_vec());
            }),
        );
    });

    let response = app
        .handle_request(
            request("GET", &format!("{ORIGIN}/cf"), &[], None),
            JsValue::UNDEFINED,
        )
        .await
        .expect("the CF dispatch answers");
    assert_eq!(response.status(), 200, "{}: case 12: status", host_name());
    assert_eq!(
        body_text(&response).await,
        "cf-ok",
        "{}: case 12: body",
        host_name()
    );
}

/// Case 13. Oracle: expected outcome. No binding is stored, and nothing panics.
#[wasm_bindgen_test]
async fn cf_dispatch_without_a_binding_stores_none() {
    let app = cf_app_with(|app| {
        app.router.add_route_cf(
            GET,
            "/db",
            CfFnHandler::new(|ctx, _req, conn| {
                let found = request_db(ctx).is_some();
                conn.set_status(200);
                conn.set_body(if found {
                    b"some".to_vec()
                } else {
                    b"none".to_vec()
                });
            }),
        );
    });

    let response = app
        .handle_request(
            request("GET", &format!("{ORIGIN}/db"), &[], None),
            JsValue::UNDEFINED,
        )
        .await
        .expect("the CF dispatch answers without a binding");
    assert_eq!(
        body_text(&response).await,
        "none",
        "{}: case 13: a handler sees no D1 database",
        host_name()
    );
}

/// Case 14. Oracle: example, workerd only. The handler finds the real local D1.
///
/// This is the one host-conditional case (invariant I5). On a host without
/// `globalThis.__CF_TEST_ENV__` it prints why it did not run and does not fail.
#[wasm_bindgen_test]
async fn cf_dispatch_sees_the_real_db_binding() {
    let Some(env) = test_env() else {
        println!(
            "skipped: __CF_TEST_ENV__ is unset (not the workerd host); case 14 needs the real D1 binding"
        );
        return;
    };

    let app = cf_app_with(|app| {
        app.router.add_route_cf(
            GET,
            "/db",
            CfFnHandler::new(|ctx, _req, conn| {
                let found = request_db(ctx).is_some();
                conn.set_status(200);
                conn.set_body(if found {
                    b"some".to_vec()
                } else {
                    b"none".to_vec()
                });
            }),
        );
    });

    let response = app
        .handle_request(request("GET", &format!("{ORIGIN}/db"), &[], None), env)
        .await
        .expect("the CF dispatch answers with the real env");
    assert_eq!(
        body_text(&response).await,
        "some",
        "{}: case 14: the handler must find the real D1 binding",
        host_name()
    );
}

/// Case 15. Oracle: example. `CfConn::into_response` keeps status and headers.
#[wasm_bindgen_test]
async fn cf_conn_into_response_preserves_status_and_headers() {
    let mut conn = CfConn::new();
    conn.set_status(418);
    conn.set_header("x-cf", "teapot");
    conn.set_body(b"short and stout".to_vec());

    let response = conn.into_response().expect("build the response");
    assert_eq!(response.status(), 418, "{}: case 15: status", host_name());
    assert_eq!(
        header(&response, "x-cf").as_deref(),
        Some("teapot"),
        "{}: case 15: header",
        host_name()
    );
    assert_eq!(
        body_text(&response).await,
        "short and stout",
        "{}: case 15: body",
        host_name()
    );
}

// ── T4: the writer path, the stream and the response (cases 16 to 20) ─

/// Case 16. Oracle: example. Status, header and body arrive as a `WasmResponse`.
#[wasm_bindgen_test]
async fn serve_writer_round_trip() {
    let mut app = HttpApp::<Arc<dyn ServeWriter>>::new_writer();
    app.router.add_route_writer(
        GET,
        "/writer",
        Arc::new(TextWriterHandler { body: "writer-ok" }),
    );

    let response = handle_writer_request(
        &app,
        writer_request(GET, &format!("{ORIGIN}/writer"), SendSafeBody::None),
    )
    .expect("the writer dispatch answers");

    assert_eq!(response.status, 200, "{}: case 16: status", host_name());
    assert_eq!(
        response.body,
        Some(b"writer-ok".to_vec()),
        "{}: case 16: body",
        host_name()
    );
    assert!(
        response
            .headers
            .iter()
            .any(|(name, value)| name.eq_ignore_ascii_case("content-type")
                && value == "text/plain"),
        "{}: case 16: content-type header, got {:?}",
        host_name(),
        response.headers
    );
}

/// Case 17. Oracle: expected outcome. The middleware's header reaches the
/// request that the handler-path receives.
#[wasm_bindgen_test]
async fn run_middleware_writer_sees_mutations() {
    let mut app = HttpApp::<Arc<dyn ServeWriter>>::new_writer();
    app.middleware(AddRequestHeader);

    let (req, short_circuit) = run_middleware_writer(
        &app,
        Ctx::host(),
        writer_request(GET, &format!("{ORIGIN}/writer"), SendSafeBody::None),
    );

    assert!(
        short_circuit.is_none(),
        "{}: case 17: the middleware continues",
        host_name()
    );
    let values = req
        .headers
        .get(&SimpleHeader::from("X-Middleware".to_string()))
        .expect("the middleware inserted its header");
    assert_eq!(
        values.first().map(String::as_str),
        Some("passed"),
        "{}: case 17: middleware value",
        host_name()
    );
}

/// Case 18. Oracle: example (limit 16, body 17). The limit is a config field.
#[wasm_bindgen_test]
async fn serve_writer_body_over_the_configured_limit_returns_413() {
    let mut app = HttpApp::<Arc<dyn ServeWriter>>::new_writer().with_max_writer_body_bytes(16);
    app.router.add_route_writer(
        POST,
        "/upload",
        Arc::new(TextWriterHandler { body: "accepted" }),
    );

    let response = handle_writer_request(
        &app,
        writer_request(
            POST,
            &format!("{ORIGIN}/upload"),
            SendSafeBody::Bytes(vec![b'x'; 17]),
        ),
    )
    .expect("the writer dispatch answers");

    assert_eq!(
        response.status,
        413,
        "{}: case 18: a 17-byte body over a 16-byte limit",
        host_name()
    );
}

/// Case 19. Oracle: example. The raw chunks split into status, headers and
/// body, in write order.
#[wasm_bindgen_test]
async fn wasm_stream_parse_raw_chunks_in_order() {
    use std::io::Write;

    let mut stream = WasmStream::new();
    stream
        .write_all(b"HTTP/1.1 201 Created\r\nX-Order: one\r\n")
        .expect("first write");
    stream
        .write_all(b"content-type: text/plain\r\n\r\n")
        .expect("second write");
    stream.write_all(b"hello").expect("third write");

    let response = stream.into_wasm_response();
    assert_eq!(response.status, 201, "{}: case 19: status", host_name());
    assert_eq!(
        response.headers,
        vec![
            ("X-Order".to_string(), "one".to_string()),
            ("content-type".to_string(), "text/plain".to_string()),
        ],
        "{}: case 19: header order",
        host_name()
    );
    assert_eq!(
        response.body,
        Some(b"hello".to_vec()),
        "{}: case 19: body",
        host_name()
    );
}

/// Case 20. Oracle: example. The builder's values are the response's values.
#[wasm_bindgen_test]
async fn wasm_response_copies_status_headers_and_body() {
    let built = WasmResponse::new(201)
        .with_header("x-built", "yes")
        .with_body(b"built body".to_vec());

    let response = from_wasm(built).expect("build the web_sys response");
    assert_eq!(response.status(), 201, "{}: case 20: status", host_name());
    assert_eq!(
        header(&response, "x-built").as_deref(),
        Some("yes"),
        "{}: case 20: header",
        host_name()
    );
    assert_eq!(
        body_text(&response).await,
        "built body",
        "{}: case 20: body",
        host_name()
    );
}
