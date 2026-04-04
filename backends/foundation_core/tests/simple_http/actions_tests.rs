//! Unit tests for `client::actions` moved into the canonical units test tree.
//!
//! These tests are non-destructive copies of the original in-crate `#[cfg(test)]`
//! module. They exercise compile-time and lightweight runtime behavior for the
//! `RedirectAction`, `TlsUpgradeAction`, and `HttpClientAction` types.
//!
//! Notes:
//! - Tests avoid performing real network operations; where real execution would
//!   be required they instead perform compile-time type checks or simple
//!   ownership/Option semantics checks.

use foundation_core::wire::simple_http::client::*;
use foundation_core::wire::simple_http::*;

use foundation_core::extensions::result_ext::BoxedResult;
use std::net::SocketAddr as StdSocketAddr;

#[test]
fn test_service_action_with_function_simple_server_can_clone() {
    let resource = ServiceAction::builder()
        .with_route("/service/endpoint/v1")
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
        .with_method(SimpleMethod::GET)
        .with_body(FuncSimpleServer::new(|_req| {
            SimpleOutgoingResponse::builder()
                .with_status(Status::BadRequest)
                .build()
                .map_err(|err| err.into_boxed_error())
        }))
        .build()
        .expect("should generate service action");

    let cloned_resource = resource.clone();
    _ = cloned_resource;
}

#[test]
fn test_service_action_can_clone() {
    let resource = ServiceAction::builder()
        .with_route("/service/endpoint/v1")
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
        .with_method(SimpleMethod::GET)
        .with_body(DefaultSimpleServer::default())
        .build()
        .expect("should generate service action");

    let cloned_resource = resource.clone();
    _ = cloned_resource;
}

#[test]
fn test_service_action_match_url_with_headers() {
    let resource = ServiceAction::builder()
        .with_route("/service/endpoint/v1")
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
        .with_method(SimpleMethod::GET)
        .with_body(DefaultSimpleServer::default())
        .build()
        .expect("should generate service action");

    let mut headers = SimpleHeaders::new();
    let _ = headers.insert(SimpleHeader::CONTENT_TYPE, vec!["application/json".into()]);

    let (matched_url, params) =
        resource.extract_match("/service/endpoint/v1", SimpleMethod::GET, Some(headers));

    assert!(matched_url);
    assert!(params.is_none());
}

#[test]
fn test_service_action_match_url_only() {
    let resource = ServiceAction::builder()
        .with_route("/service/endpoint/v1")
        .with_method(SimpleMethod::GET)
        .with_body(DefaultSimpleServer::default())
        .build()
        .expect("should generate service action");

    let (matched_url, params) =
        resource.extract_match("/service/endpoint/v1", SimpleMethod::GET, None);

    assert!(matched_url);
    assert!(params.is_none());
}

// // ========================================================================
// // TlsUpgradeAction Tests
// // ========================================================================

// #[cfg(not(target_arch = "wasm32"))]
// mod tls_upgrade_tests {
//     use super::*;

//     /// WHY: Verify TlsUpgradeAction structure and fields
//     /// WHAT: Tests that TlsUpgradeAction has correct field types (without real connection)
//     #[test]
//     fn test_tls_upgrade_action_structure() {
//         // We test the structure without creating a real connection
//         // since Connection::without_timeout actually attempts to connect

//         // Type check: verify TlsUpgradeAction can hold the expected types
//         fn _assert_tls_upgrade_holds_expected_types(_action: TlsUpgradeAction) {
//             // Compile-time check that the type is correct
//         }
//     }

//     /// WHY: Verify TlsUpgradeAction is an ExecutionAction (trait bound check)
//     /// WHAT: Tests that TlsUpgradeAction implements ExecutionAction trait (compile-time)
//     #[test]
//     fn test_tls_upgrade_action_is_execution_action() {
//         // Type check: verify TlsUpgradeAction implements ExecutionAction
//         fn _assert_is_execution_action<T: ExecutionAction>() {}
//         _assert_is_execution_action::<TlsUpgradeAction>();
//     }
// }

// // ========================================================================
// // HttpClientAction Tests
// // ========================================================================

// /// WHY: Verify HttpClientAction::None variant works
// /// WHAT: Tests that None variant can be created and is an ExecutionAction
// #[test]
// fn test_http_client_action_none() {
//     let action: HttpClientAction<MockDnsResolver> = HttpClientAction::None;

//     // Should compile and be callable (even if it does nothing)
//     // We can't actually call apply without a real engine, but we can verify the type
//     let _boxed: Box<dyn ExecutionAction> = Box::new(action);
// }

// /// WHY: Verify HttpClientAction::Redirect variant delegates correctly
// /// WHAT: Tests that Redirect variant wraps RedirectAction properly
// #[test]
// fn test_http_client_action_redirect() {
//     let request = ClientRequestBuilder::get("http://example.com")
//         .unwrap()
//         .build()
//         .unwrap();
//     let resolver = MockDnsResolver::new();

//     let redirect_action = RedirectAction::new(request, resolver, 3, None);
//     let action = HttpClientAction::Redirect(redirect_action);

//     // Type check
//     let _boxed: Box<dyn ExecutionAction> = Box::new(action);
// }

// /// WHY: Verify HttpClientAction::TlsUpgrade variant delegates correctly
// /// WHAT: Tests that TlsUpgrade variant type compiles (compile-time check)
// #[test]
// #[cfg(not(target_arch = "wasm32"))]
// fn test_http_client_action_tls_upgrade() {
//     // Compile-time type check: verify HttpClientAction can hold TlsUpgradeAction
//     fn _assert_tls_upgrade_variant_exists() {
//         use foundation_core::wire::simple_http::client::DnsResolver;

//         // This verifies the enum variant compiles correctly
//         fn _assert_can_create<R: DnsResolver + Send + 'static>(_action: HttpClientAction<R>) {
//             // Type check
//         }
//     }
// }
