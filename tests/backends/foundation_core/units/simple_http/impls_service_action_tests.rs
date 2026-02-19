#![allow(unused)]

// Non-destructive unit tests for `ServiceAction` moved into the canonical
// units test tree.
//
// These tests are conservative: they exercise the public builder surface that
// the original in-crate tests referenced (route/header/method chaining) and
// include compile-time trait assertions (e.g., `Clone`) that the original
// test-suite relied upon. They avoid performing any I/O or runtime networking
// operations so they are fast and suitable for unit test execution.
//
// NOTE: The original impls module included richer runtime tests that are kept
// in the integration test suite; these unit tests focus on API shape and basic
// trait properties.

use foundation_core::wire::simple_http::{ServiceAction, SimpleHeader, SimpleMethod};

/// Sanity-check: basic builder chaining compiles.
///
/// This mirrors the original test that constructed a `ServiceAction` using
/// the fluent builder API and ensured the builder methods exist and are
/// chainable. The test is intentionally minimal at runtime (only asserts true)
/// because the principal goal is to verify the public API surface is intact.
#[test]
fn test_service_action_builder_chain_compiles() {
    // Chain a few common builder methods that the original tests used.
    // We don't call into network behavior here; just validate the builder API.
    let _builder = ServiceAction::builder()
        .with_route("/service/endpoint/v1")
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
        .with_method(SimpleMethod::GET);

    // If the chain above compiles, the public API surface exists as expected.
    assert!(true);
}

/// Compile-time assertion: `ServiceAction` is `Clone`.
///
/// The original suite relied on being able to clone `ServiceAction` instances
/// in certain server/composition scenarios. This test enforces that contract
/// at compile time. If `ServiceAction` is not `Clone`, compilation will fail
/// and the test will not run.
#[test]
fn test_service_action_is_clone() {
    fn assert_clone<T: Clone>() {}
    assert_clone::<ServiceAction>();
}

/// Smoke test: ensure common header and method enums are usable from the public API.
///
/// This is a lightweight runtime check verifying that the `SimpleHeader` and
/// `SimpleMethod` symbols are accessible from the public crate surface and can
/// be used in builder calls.
#[test]
fn test_service_action_header_and_method_accessible() {
    // Use the enums in a short-lived builder chain to ensure they are exported
    // and that the builder accepts them.
    let _ = ServiceAction::builder()
        .with_route("/health")
        .add_header(SimpleHeader::HOST, "localhost")
        .with_method(SimpleMethod::HEAD);

    // No runtime assertions necessary beyond successful compilation/execution.
    assert!(true);
}
