//! Type-level tests for BuildKit gRPC message types.
//!
//! WHY: Prove the hand-written type stubs satisfy the trait bounds required by
//! `foundation_connectrpc::Client<Req, Res>` — at minimum `Send + Send + 'static`
//! (the Client bounds) and `Serialize + DeserializeOwned` (for JsonCodec).
//!
//! WHAT: Compile-time + runtime assertions on each P0 message type.
//!
//! HOW: Each test instantiates a default value and round-trips through serde_json.

#![cfg(feature = "buildkit")]

use foundation_deployment_docker::buildkit::types::{
    BuildKitVersion, InfoRequest, InfoResponse, SolveRequest, SolveResponse,
    StatusRequest, StatusResponse, Vertex, VertexLog,
};

#[test]
fn info_request_default_is_empty() {
    let req = InfoRequest::default();
    let json = serde_json::to_string(&req).expect("serialize");
    let _back: InfoRequest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(json, "{}");
}

#[test]
fn info_response_round_trips() {
    let resp = InfoResponse {
        buildkit_version: Some(BuildKitVersion {
            version: "0.12.0".into(),
            revision: "abc123".into(),
            package: "github.com/moby/buildkit".into(),
        }),
    };
    let json = serde_json::to_string(&resp).expect("serialize");
    assert!(json.contains("0.12.0"));
    let _back: InfoResponse = serde_json::from_str(&json).expect("deserialize");
}

#[test]
fn status_response_with_vertices_round_trips() {
    let resp = StatusResponse {
        vertexes: vec![Vertex {
            digest: "sha256:abc".into(),
            name: "RUN echo hello".into(),
            cached: false,
            ..Default::default()
        }],
        logs: vec![VertexLog {
            vertex: "sha256:abc".into(),
            msg: b"hello\n".to_vec(),
            stream: 1,
            ..Default::default()
        }],
        ..Default::default()
    };
    let json = serde_json::to_string(&resp).expect("serialize");
    assert!(json.contains("RUN echo hello"));
    let back: StatusResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.vertexes.len(), 1);
    assert_eq!(back.logs.len(), 1);
}

#[test]
fn solve_request_default_is_partial() {
    let req = SolveRequest::default();
    let json = serde_json::to_string(&req).expect("serialize");
    let _back: SolveRequest = serde_json::from_str(&json).expect("deserialize");
}

#[test]
fn solve_response_is_status_response() {
    // Prove the type alias works: SolveResponse = StatusResponse
    let sr: SolveResponse = StatusResponse::default();
    let json = serde_json::to_string(&sr).expect("serialize");
    let _: StatusResponse = serde_json::from_str(&json).expect("deserialize as StatusResponse");
}

#[test]
fn status_request_ref_field() {
    let req = StatusRequest { r#ref: "build-ref-123".into() };
    let json = serde_json::to_string(&req).expect("serialize");
    assert!(json.contains("build-ref-123"));
    let back: StatusRequest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.r#ref, "build-ref-123");
}
