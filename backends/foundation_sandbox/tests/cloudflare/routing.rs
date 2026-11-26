//! The routing rule and the bearer-token check of the Worker entry (feature 08,
//! section 6.1 and invariant I1).

use std::sync::Arc;

use foundation_sandbox::shared::cloudflare::control::remove_index_on_stream_error;
use foundation_sandbox::shared::cloudflare::routing::{
    route, RouteTarget, CONTAINER_SCOPED_METHODS, CONTAINER_SERVICE_PREFIX, SANDBOX_SERVICE_PREFIX,
    SCOPED_METHODS,
};
use foundation_sandbox::shared::cloudflare::testing::MemoryIndex;
use foundation_sandbox::shared::cloudflare::SandboxIndex;
use foundation_sandbox::shared::control::{authorize, SandboxInfo, SandboxState};
use foundation_sandbox::shared::error::{SandboxError, SandboxResult};
use foundation_sandbox::shared::ids::{DriverId, SandboxId};
use foundation_sandbox::shared::{BoxByteStream, Secret};
use futures::StreamExt;
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

/// The path of one method of `SandboxService`.
fn path(method: &str) -> String {
    format!("{SANDBOX_SERVICE_PREFIX}{method}")
}

/// The JSON body of one scoped request.
fn scoped_body(id: &str) -> Vec<u8> {
    format!(r#"{{"sandbox_id":"{id}"}}"#).into_bytes()
}

/// The path of one method of `ContainerService`.
fn container_path(method: &str) -> String {
    format!("{CONTAINER_SERVICE_PREFIX}{method}")
}

/// The JSON body of one container request.
fn container_body(id: &str) -> Vec<u8> {
    format!(r#"{{"container_id":"{id}"}}"#).into_bytes()
}

/// The error that a failed route returned.
fn error_of(result: SandboxResult<RouteTarget>) -> SandboxError {
    result
        .expect_err("the route must fail")
        .current_context()
        .clone()
}

/// A valid identifier for the examples.
fn id(text: &str) -> SandboxId {
    text.parse().expect("the example identifier is valid")
}

/// Every row of the routing table in feature 08, section 6.1.
#[test]
fn route_table() {
    assert_eq!(route(&path("Probe"), b"").unwrap(), RouteTarget::Probe);
    assert_eq!(route(&path("List"), b"").unwrap(), RouteTarget::List);
    assert_eq!(
        route(&path("Create"), b"").unwrap(),
        RouteTarget::NewSandbox
    );
    let body = scoped_body("000000000001");
    for method in SCOPED_METHODS {
        assert_eq!(
            route(&path(method), &body).unwrap(),
            RouteTarget::Sandbox(id("000000000001")),
            "method {method}"
        );
    }
}

/// A scoped call reads only `sandbox_id`; every other body field is ignored.
#[test]
fn route_reads_only_the_identifier() {
    let body = br#"{"sandbox_id":"000000000002","spec":{"vcpus":4}}"#;
    assert_eq!(
        route(&path("Exec"), body).unwrap(),
        RouteTarget::Sandbox(id("000000000002"))
    );
}

/// An identifier that is not a `SandboxId` is refused before any Durable Object
/// is reached.
#[test]
fn route_bad_id() {
    let error = error_of(route(&path("Get"), &scoped_body("../x")));
    match error {
        SandboxError::InvalidSpec { field, .. } => assert_eq!(field, "sandbox_id"),
        other => panic!("expected InvalidSpec, got {other:?}"),
    }
}

/// A body that is not JSON, or that has no `sandbox_id`, is an `InvalidSpec`
/// with the same field.
#[test]
fn route_bad_body() {
    for body in [&b"not json"[..], &b"{}"[..], &br#"{"sandbox_id":5}"#[..]] {
        let error = error_of(route(&path("Get"), body));
        match error {
            SandboxError::InvalidSpec { field, .. } => assert_eq!(field, "sandbox_id"),
            other => panic!("expected InvalidSpec, got {other:?}"),
        }
    }
}

/// A path that is not a method of `SandboxService` is `NotFound { what: "method" }`.
#[test]
fn route_unknown_method() {
    for path in [
        path("Unknown"),
        format!("{SANDBOX_SERVICE_PREFIX}Get/extra"),
        "/index/x".to_string(),
        "/ewe.sandbox.container.v1.ContainerService/Unknown".to_string(),
        String::new(),
    ] {
        match error_of(route(&path, b"")) {
            SandboxError::NotFound { what } => assert_eq!(what, "method"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}

/// The token check of invariant I1: a missing header, another scheme and a
/// wrong token fail; the right token passes.
#[test]
fn check_bearer_cases() {
    let token = Secret::new("test");
    assert!(authorize(None, &token).is_err(), "a missing header");
    assert!(
        authorize(Some("Basic dGVzdA=="), &token).is_err(),
        "another scheme"
    );
    assert!(
        authorize(Some("Bearer wrong"), &token).is_err(),
        "a wrong token"
    );
    assert!(authorize(Some("Bearer test"), &token).is_ok(), "the token");
}

/// Every `ContainerService` method that names a container reaches the same
/// Durable Object as a scoped sandbox call.
#[test]
fn route_container_table() {
    for method in CONTAINER_SCOPED_METHODS {
        assert_eq!(
            route(&container_path(method), &container_body("000000000001")).unwrap(),
            RouteTarget::Container(id("000000000001")),
            "method {method}"
        );
    }
}

/// `Start` and `List` name no container, and a bad identifier is refused.
#[test]
fn route_container_without_id() {
    for method in ["Start", "List"] {
        match error_of(route(&container_path(method), b"{}")) {
            SandboxError::InvalidSpec { field, .. } => assert_eq!(field, "container_id"),
            other => panic!("expected InvalidSpec, got {other:?}"),
        }
    }
    match error_of(route(
        &container_path("Get"),
        &container_body("not-an-identifier"),
    )) {
        SandboxError::InvalidSpec { field, .. } => assert_eq!(field, "container_id"),
        other => panic!("expected InvalidSpec, got {other:?}"),
    }
}

/// A stream error after the head removes the index entry (invariant I6).
#[test]
fn stream_error_removes_index_entry() {
    let id = id("000000000003");
    let index = Arc::new(MemoryIndex::new());
    let info = SandboxInfo {
        id: id.to_string(),
        driver: DriverId::CloudflareContainer,
        state: SandboxState::Ready,
        created_unix_ms: 0,
        expires_at_unix_ms: None,
        slave: None,
        manifest: None,
    };
    futures::executor::block_on(SandboxIndex::put(&*index, info)).expect("the entry");
    assert!(index.get(&id).is_some());

    let body: BoxByteStream = Box::pin(futures::stream::once(async {
        Err(SandboxError::StreamLost.into())
    }));
    let wrapped = remove_index_on_stream_error(
        body,
        Arc::clone(&index) as Arc<dyn SandboxIndex>,
        id.clone(),
    );
    let items = futures::executor::block_on(wrapped.collect::<Vec<_>>());
    assert_eq!(items.len(), 1);
    assert!(items[0].is_err(), "the error still reaches the caller");
    assert!(
        index.get(&id).is_none(),
        "the stream error removed the entry"
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        rng_seed: RngSeed::Fixed(0x5eed_0008),
        cases: 64,
        .. ProptestConfig::default()
    })]

    /// Any valid identifier in a scoped body routes to that identifier (law:
    /// identity).
    #[test]
    fn route_property(id_text in "[0-9a-f]{12}") {
        for method in SCOPED_METHODS {
            let target = route(&path(method), &scoped_body(&id_text))
                .expect("a valid identifier routes");
            prop_assert_eq!(target, RouteTarget::Sandbox(id(&id_text)));
        }
        for method in CONTAINER_SCOPED_METHODS {
            let target = route(&container_path(method), &container_body(&id_text))
                .expect("a valid identifier routes");
            prop_assert_eq!(target, RouteTarget::Container(id(&id_text)));
        }
    }
}
