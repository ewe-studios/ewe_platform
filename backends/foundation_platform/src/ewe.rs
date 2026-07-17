//! `ewe://` custom protocol handler, transport lanes, and protocol selection.
//!
//! One scheme (`ewe://`). Transport implied by `RouteSource`.
//! Protocol selection: decision hint → `?proto=` query → content detection → default.

use std::collections::HashMap;

use foundation_ui_traits::*;
use tauri::{
    http::{header, Request, Response, StatusCode},
    Manager, UriSchemeContext,
};

use crate::session::PlatformSession;

// ── ewe:// registration ──────────────────────────────────────────────

/// Register `ewe://` with Tauri. One scheme, no sub-schemes.
pub fn register_ewe_protocol<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
) -> tauri::Builder<R> {
    builder.register_uri_scheme_protocol("ewe", ewe_handler::<R>)
}

/// The handler for every `ewe://` request.
fn ewe_handler<R: tauri::Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let uri = request.uri().to_string();

    // 1. Parse URI
    let ewe_url = match EweUrl::parse(&uri) {
        Ok(u) => u,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(e.into_bytes())
                .unwrap();
        }
    };

    // 2. Resolve route through session handler chain
    let session = ctx.app_handle().state::<std::sync::Arc<PlatformSession>>();
    let intent = NavigationIntent {
        url: uri,
        method: method_from_request(&request),
        source: IntentSource::LinkClick,
        referrer: session.active_page_identity().map(|p| p.route),
    };
    let decision = session.resolve_route(&intent);

    // 3-5. Cache, presentation, backend query — rest of execution contract
    let content = format!(
        "route {} resolved to {:?} (profile {:?})",
        ewe_url.path, decision.source, decision.profile
    );

    // 6. Protocol selection
    let protocol = select_protocol(
        &decision.protocol,
        ewe_url.query_params.get("proto"),
        content.as_bytes(),
    );

    // 7. Encode and respond
    let (body, content_type) = encode_protocol(&protocol, content.as_bytes());
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header("Access-Control-Allow-Origin", "ewe://localhost")
        .body(body)
        .unwrap()
}

// ── URL parsing ───────────────────────────────────────────────────────

struct EweUrl {
    path: String,
    query_params: HashMap<String, String>,
}

impl EweUrl {
    fn parse(uri: &str) -> Result<Self, String> {
        let without_scheme = uri
            .strip_prefix("ewe://localhost")
            .ok_or_else(|| format!("not an ewe:// URL: {uri}"))?;

        let (path, query) = match without_scheme.split_once('?') {
            Some((p, q)) => (p.to_string(), q.to_string()),
            None => (without_scheme.to_string(), String::new()),
        };

        let query_params: HashMap<String, String> = query
            .split('&')
            .filter(|s| !s.is_empty())
            .filter_map(|pair| pair.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        Ok(EweUrl { path, query_params })
    }
}

fn method_from_request(request: &Request<Vec<u8>>) -> Method {
    match request.method().as_str() {
        "POST" => Method::Post,
        "PUT" => Method::Put,
        "PATCH" => Method::Patch,
        "DELETE" => Method::Delete,
        _ => Method::Get,
    }
}

// ── Protocol selection ────────────────────────────────────────────────

/// Select wire protocol. Priority: decision hint → query param → detect → default.
pub fn select_protocol(
    decision_hint: &ProtocolHint,
    query_hint: Option<&String>,
    content: &[u8],
) -> Protocol {
    if *decision_hint != ProtocolHint::Default {
        return protocol_from_hint(decision_hint);
    }
    if let Some(hint) = query_hint {
        if let Some(p) = protocol_from_query(hint) {
            return p;
        }
    }
    if let Some(p) = detect_protocol(content) {
        return p;
    }
    Protocol::Columnar
}

fn protocol_from_hint(hint: &ProtocolHint) -> Protocol {
    match hint {
        ProtocolHint::Columnar => Protocol::Columnar,
        ProtocolHint::Arrow => Protocol::Arrow,
        ProtocolHint::ArrowIpc => Protocol::ArrowIpc,
        ProtocolHint::Json => Protocol::Json,
        ProtocolHint::Html => Protocol::Html,
        ProtocolHint::Default => Protocol::Columnar,
    }
}

fn protocol_from_query(hint: &str) -> Option<Protocol> {
    match hint {
        "columnar" => Some(Protocol::Columnar),
        "arrow" => Some(Protocol::Arrow),
        "arrow-ipc" => Some(Protocol::ArrowIpc),
        "json" => Some(Protocol::Json),
        "html" => Some(Protocol::Html),
        _ => None,
    }
}

fn detect_protocol(content: &[u8]) -> Option<Protocol> {
    if content.is_empty() {
        return None;
    }
    if content.len() >= 8 && &content[..4] == b"\xFF\xFF\xFF\xFF" {
        return Some(Protocol::ArrowIpc);
    }
    if content[0] == b'{' || content[0] == b'[' {
        return Some(Protocol::Json);
    }
    if content[0] == b'<' {
        return Some(Protocol::Html);
    }
    None
}

// ── Protocol encoding ─────────────────────────────────────────────────

/// Encode bytes with the correct Content-Type.
pub fn encode_protocol(protocol: &Protocol, content: &[u8]) -> (Vec<u8>, &'static str) {
    (content.to_vec(), protocol.content_type())
}

// ── Transport lanes ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    CustomProtocol,
    CommandIpc,
    Events,
    NativeShellIpc,
    LocalServer,
    RemoteSseWs,
    BrowserFetch,
}

/// Transport implied by route source.
pub fn transport_for_source(source: RouteSource) -> Transport {
    match source {
        RouteSource::WebviewApp => Transport::CustomProtocol,
        RouteSource::IpcShell => Transport::CommandIpc,
        RouteSource::RemoteServer => Transport::CustomProtocol,
    }
}

// ── Tests ────────────────────────────────────────────────────────────

// NOTE: Tests kept inline because: Tests EweUrl parsing, protocol detection bytes, query hint mapping—private URL parser internals.
#[cfg(test)]
mod tests {
    use super::*;

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
}
