//! Request ID middleware tests (moved from `src/shared/middleware/request_id.rs`).

use foundation_compact::uuid::Uuid;
use foundation_http::shared::handlers::RequestIdMiddleware;
use foundation_http::shared::middleware::{MiddlewareResult, RequestMiddleware};
use foundation_http::{SimpleHeader, SimpleIncomingRequest, SimpleMethod};

fn request() -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_plain_url("/")
        .with_method(SimpleMethod::GET)
        .build()
        .expect("valid request")
}

fn values_for<'a>(req: &'a SimpleIncomingRequest, name: &str) -> Option<&'a Vec<String>> {
    req.headers.iter().find_map(|(header, values)| {
        if format!("{header}").eq_ignore_ascii_case(name) {
            Some(values)
        } else {
            None
        }
    })
}

#[test]
fn generates_a_version_4_id_when_the_client_sends_none() {
    let mw = RequestIdMiddleware;
    let ctx = foundation_context::Ctx::host();
    let mut req = request();

    assert!(matches!(
        mw.handle(&ctx, &mut req),
        MiddlewareResult::Continue
    ));

    let values = values_for(&req, "x-request-id").expect("the middleware adds an ID");
    assert_eq!(values.len(), 1);
    let id = Uuid::parse_str(&values[0]).expect("the ID is a UUID");
    assert_eq!(id.get_version_num(), 4);
}

#[test]
fn keeps_a_client_supplied_id() {
    let mw = RequestIdMiddleware;
    let ctx = foundation_context::Ctx::host();
    let mut req = SimpleIncomingRequest::builder()
        .with_plain_url("/")
        .with_method(SimpleMethod::GET)
        .add_header(SimpleHeader::custom("X-Request-Id"), "client-id")
        .build()
        .expect("valid request");

    assert!(matches!(
        mw.handle(&ctx, &mut req),
        MiddlewareResult::Continue
    ));

    let values = values_for(&req, "x-request-id").expect("the client ID stays");
    assert_eq!(values.as_slice(), ["client-id"]);
    let count = req
        .headers
        .keys()
        .filter(|header| format!("{header}").eq_ignore_ascii_case("x-request-id"))
        .count();
    assert_eq!(count, 1, "the middleware must not add a second ID header");
}

#[test]
fn matches_the_client_header_case_insensitively() {
    let mw = RequestIdMiddleware;
    let ctx = foundation_context::Ctx::host();
    let mut req = SimpleIncomingRequest::builder()
        .with_plain_url("/")
        .with_method(SimpleMethod::GET)
        .add_header(SimpleHeader::custom("x-REQUEST-id"), "client-id")
        .build()
        .expect("valid request");

    assert!(matches!(
        mw.handle(&ctx, &mut req),
        MiddlewareResult::Continue
    ));

    let count = req
        .headers
        .keys()
        .filter(|header| format!("{header}").eq_ignore_ascii_case("x-request-id"))
        .count();
    assert_eq!(count, 1, "a different case is still the same header");
}
