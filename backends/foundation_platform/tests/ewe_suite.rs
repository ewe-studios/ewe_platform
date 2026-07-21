//! Tests for ewe:// URL parsing, protocol selection, and transport mapping.

use foundation_platform::ewe::{EweUrl, method_from_request, protocol_from_hint, protocol_from_query};
use foundation_platform::*;
use tauri::http::Request;

#[test]
fn parse_basic_ewe_url() {
    let url = EweUrl::parse("ewe://localhost/app/items").unwrap();
    assert_eq!(url.path, "/app/items");
    assert!(url.query_params.is_empty());
}

#[test]
fn parse_ewe_url_with_query() {
    let url = EweUrl::parse("ewe://localhost/app/items?proto=arrow&cache=stale-while-revalidate").unwrap();
    assert_eq!(url.path, "/app/items");
    assert_eq!(url.query_params.get("proto").unwrap(), "arrow");
}

#[test]
fn parse_rejects_non_ewe() {
    assert!(EweUrl::parse("https://example.com/app").is_err());
}

// ── Protocol selection ───────────────────────────────────────

#[test]
fn select_default_columnar() {
    assert_eq!(select_protocol(&ProtocolHint::Default, None, b"hello"), Protocol::Columnar);
}

#[test]
fn select_respects_decision_hint() {
    assert_eq!(select_protocol(&ProtocolHint::Json, None, b"hello"), Protocol::Json);
}

#[test]
fn select_respects_query_hint() {
    assert_eq!(
        select_protocol(&ProtocolHint::Default, Some(&"arrow".into()), b"hello"),
        Protocol::Arrow
    );
}

#[test]
fn select_decision_overrides_query() {
    assert_eq!(
        select_protocol(&ProtocolHint::Json, Some(&"arrow".into()), b"hello"),
        Protocol::Json
    );
}

#[test]
fn select_detects_json() {
    assert_eq!(
        select_protocol(&ProtocolHint::Default, None, b"{\"k\":\"v\"}"),
        Protocol::Json
    );
}

#[test]
fn select_detects_html() {
    assert_eq!(
        select_protocol(&ProtocolHint::Default, None, b"<html>"),
        Protocol::Html
    );
}

#[test]
fn select_detects_arrow_ipc() {
    let mut c = vec![0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0];
    c.extend_from_slice(b"payload");
    assert_eq!(select_protocol(&ProtocolHint::Default, None, &c), Protocol::ArrowIpc);
}

#[test]
fn select_unknown_falls_to_columnar() {
    assert_eq!(select_protocol(&ProtocolHint::Default, None, b"\x00\x01"), Protocol::Columnar);
}

#[test]
fn all_protocol_hints_map_correctly() {
    assert_eq!(protocol_from_hint(&ProtocolHint::Default), Protocol::Columnar);
    assert_eq!(protocol_from_hint(&ProtocolHint::Arrow), Protocol::Arrow);
    assert_eq!(protocol_from_hint(&ProtocolHint::ArrowIpc), Protocol::ArrowIpc);
    assert_eq!(protocol_from_hint(&ProtocolHint::Json), Protocol::Json);
    assert_eq!(protocol_from_hint(&ProtocolHint::Html), Protocol::Html);
}

#[test]
fn all_query_hints() {
    assert_eq!(protocol_from_query("columnar"), Some(Protocol::Columnar));
    assert_eq!(protocol_from_query("arrow"), Some(Protocol::Arrow));
    assert_eq!(protocol_from_query("arrow-ipc"), Some(Protocol::ArrowIpc));
    assert_eq!(protocol_from_query("json"), Some(Protocol::Json));
    assert_eq!(protocol_from_query("html"), Some(Protocol::Html));
    assert_eq!(protocol_from_query("xml"), None);
}

// ── Encoding ──────────────────────────────────────────────────

#[test]
fn encode_preserves_bytes() {
    let (bytes, ct) = encode_protocol(&Protocol::Columnar, b"hello");
    assert_eq!(bytes, b"hello");
    assert_eq!(ct, "application/primal-columnar");
}

#[test]
fn encode_all_content_types() {
    assert_eq!(encode_protocol(&Protocol::Columnar, b"").1, "application/primal-columnar");
    assert_eq!(encode_protocol(&Protocol::Arrow, b"").1, "application/primal-arrow");
    assert_eq!(encode_protocol(&Protocol::ArrowIpc, b"").1, "application/vnd.apache.arrow.stream");
    assert_eq!(encode_protocol(&Protocol::Json, b"").1, "application/primal-json");
    assert_eq!(encode_protocol(&Protocol::Html, b"").1, "text/html; charset=utf-8");
    assert_eq!(encode_protocol(&Protocol::CustomBinary, b"").1, "application/primal-binary");
}

// ── Transport ─────────────────────────────────────────────────

#[test]
fn source_maps_to_transport() {
    assert_eq!(transport_for_source(RouteSource::WebviewApp), Transport::CustomProtocol);
    assert_eq!(transport_for_source(RouteSource::IpcShell), Transport::CommandIpc);
    assert_eq!(transport_for_source(RouteSource::RemoteServer), Transport::CustomProtocol);
}

// ── Method detection ──────────────────────────────────────────

#[test]
fn method_get() {
    let req = Request::builder().method("GET").uri("ewe://localhost/app").body(vec![]).unwrap();
    assert_eq!(method_from_request(&req), Method::Get);
}

#[test]
fn method_post() {
    let req = Request::builder().method("POST").uri("ewe://localhost/app").body(vec![]).unwrap();
    assert_eq!(method_from_request(&req), Method::Post);
}
