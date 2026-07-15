//! Type-level tests for the generated BuildKit gRPC message types.
//!
//! WHY: The BuildKit client drives `foundation_connectrpc::Client<Req, Res>`
//! with `ProcedureCodecs::defaults()`, whose bound is `buffa::Message +
//! Serialize + DeserializeOwned`. These tests prove the buffa-generated Control
//! message types satisfy that bound and round-trip on both the proto wire and
//! the Connect JSON wire — i.e. the client really speaks gRPC/proto to
//! buildkitd, not a JSON approximation.
//!
//! HOW: A generic `assert_codec_ready::<T>()` pins the exact trait bound
//! `defaults()` needs; the round-trip tests exercise `buffa::Message` encode /
//! decode and serde on real values.

#![cfg(feature = "buildkit")]

use buffa::Message;
use serde::{de::DeserializeOwned, Serialize};

use foundation_deployment_docker::buildkit::types::{
    InfoRequest, InfoResponse, SolveRequest, SolveResponse, StatusRequest, StatusResponse,
};

/// Compile-time proof that `T` satisfies exactly the bound `ProcedureCodecs::defaults()` imposes.
fn assert_codec_ready<T: Message + Serialize + DeserializeOwned + Default + 'static>() {}

#[test]
fn control_types_satisfy_defaults_bound() {
    assert_codec_ready::<InfoRequest>();
    assert_codec_ready::<InfoResponse>();
    assert_codec_ready::<SolveRequest>();
    assert_codec_ready::<SolveResponse>();
    assert_codec_ready::<StatusRequest>();
    assert_codec_ready::<StatusResponse>();
}

#[test]
fn info_request_proto_round_trips() {
    let req = InfoRequest::default();
    let bytes = req.encode_to_vec();
    let back = InfoRequest::decode_from_slice(&bytes).expect("proto decode");
    // An empty message encodes to zero bytes on the proto wire.
    assert!(bytes.is_empty());
    let _ = back;
}

#[test]
fn status_request_ref_round_trips_proto_and_json() {
    let req = StatusRequest { Ref: "build-ref-123".into(), ..Default::default() };

    // Proto wire round-trip.
    let bytes = req.encode_to_vec();
    let back = StatusRequest::decode_from_slice(&bytes).expect("proto decode");
    assert_eq!(back.Ref, "build-ref-123");

    // Connect JSON wire round-trip (field is proto-named `Ref`).
    let json = serde_json::to_string(&req).expect("serialize");
    assert!(json.contains("build-ref-123"), "json: {json}");
    let from_json: StatusRequest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(from_json.Ref, "build-ref-123");
}

#[test]
fn solve_request_frontend_round_trips_proto() {
    let mut req = SolveRequest::default();
    req.Frontend = "dockerfile.v0".into();
    let bytes = req.encode_to_vec();
    let back = SolveRequest::decode_from_slice(&bytes).expect("proto decode");
    assert_eq!(back.Frontend, "dockerfile.v0");
}

#[test]
fn status_response_default_round_trips_proto() {
    let resp = StatusResponse::default();
    let bytes = resp.encode_to_vec();
    let _back = StatusResponse::decode_from_slice(&bytes).expect("proto decode");
}
