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

    // 1. Validate URI is a well-formed ewe:// URL
    let _ewe_url = match EweUrl::parse(&uri) {
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

    // 3-9. Run the full execution contract through the session
    // Cache check → backend query → protocol selection → encoding → navigation record
    let (body, content_type) = session.execute_decision(&decision, &intent);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type.as_str())
        .header("Access-Control-Allow-Origin", "ewe://localhost")
        .body(body)
        .unwrap()
}

// ── URL parsing ───────────────────────────────────────────────────────

#[allow(dead_code)]
struct EweUrl {
    path: String,
    query_params: HashMap<String, String>,
}

impl EweUrl {
    fn parse(uri: &str) -> Result<Self, String> {
        // Accept ewe://<any-host>/path — strip the scheme+authority.
        let without_scheme = uri
            .strip_prefix("ewe://")
            .ok_or_else(|| format!("not an ewe:// URL: {uri}"))?;

        // Find the first '/' after the host — that's where the path starts.
        let path_start = without_scheme.find('/').unwrap_or(without_scheme.len());
        let host_and_path = &without_scheme[path_start..];
        let path_and_query = if host_and_path.is_empty() { "/" } else { host_and_path };

        let (path, query) = match path_and_query.split_once('?') {
            Some((p, q)) => (p.to_string(), q.to_string()),
            None => (path_and_query.to_string(), String::new()),
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

// ── Mode page rendering ─────────────────────────────────────────────────

/// Return a CSS class + label for the route's source.
#[allow(dead_code)]
fn mode_badge_for(decision: &RouteDecision) -> (&'static str, &'static str) {
    match decision.source {
        RouteSource::WebviewApp => ("wasm", "🟢 App — WASM in WebView"),
        RouteSource::IpcShell => ("ipc", "🟠 IPC — Native Shell"),
        RouteSource::RemoteServer => ("remote", "🟣 Remote — Server Fetch"),
    }
}

/// Build a self-contained HTML page showing which mode was activated,
/// the route, and the raw response payload.
#[allow(dead_code)]
fn build_mode_page(
    path: &str,
    (cls, label): &(&str, &str),
    decision: &RouteDecision,
    body_str: &str,
) -> String {
    // Escape the body for safe embedding in HTML
    let escaped_body = html_escape(body_str);
    let profile = format!("{:?}", decision.profile);
    let cache = format!("{:?}", decision.cache_policy);
    let proto = format!("{:?}", decision.protocol);

    format!(
        r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>ewe:// — {path}</title>
<style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{font-family:system-ui,sans-serif;padding:12px;background:#0a0a1a;color:#ccd6f6;min-height:100vh}}
h1{{font-size:20px;color:#64ffda;margin-bottom:4px}}
.badge{{font-size:12px;padding:3px 8px;border-radius:4px;display:inline-block;margin-bottom:10px}}
.wasm{{background:#1a3a2a;color:#64ffda}}
.ipc{{background:#3a2a1a;color:#ffb86c}}
.remote{{background:#2a1a3a;color:#bd93f9}}
.card{{background:#112240;border:1px solid #233554;border-radius:8px;padding:10px;margin:8px 0}}
.card h3{{font-size:13px;color:#64ffda;margin-bottom:4px}}
.pills{{display:flex;gap:6px;flex-wrap:wrap;margin:4px 0}}
.pill{{font-size:10px;padding:2px 8px;border-radius:10px;background:#0a1a2a;color:#8892b0;border:1px solid #233554}}
.payload{{font-size:11px;color:#a8b2d1;word-break:break-all;white-space:pre-wrap;max-height:300px;overflow-y:auto;background:#0a0e1a;padding:8px;border-radius:4px;margin-top:6px}}
nav a{{font-size:12px;padding:5px 10px;border:1px solid #233554;border-radius:4px;background:#112240;color:#64ffda;text-decoration:none;margin-right:4px;display:inline-block;margin-bottom:4px}}
nav a:hover{{background:#233554}}
</style></head><body>
<h1>ewe://{path}</h1>
<div class="badge {cls}">{label}</div>

<nav>
<a href="ewe://localhost/app/home">🏠 App (WASM)</a>
<a href="ewe://localhost/api/status">⚡ API (IPC)</a>
<a href="ewe://localhost/remote/news">🌐 Remote</a>
<a href="ewe://localhost/cached/data">💾 Cached</a>
<a href="ewe://localhost/auth/login">🔐 Auth</a>
</nav>

<div class="card">
<h3>Route Decision</h3>
<div class="pills">
<span class="pill">profile: {profile}</span>
<span class="pill">cache: {cache}</span>
<span class="pill">protocol: {proto}</span>
</div>
</div>

<div class="card">
<h3>Backend Response</h3>
<div class="payload">{escaped_body}</div>
</div>

<p style="font-size:10px;color:#445566;margin-top:12px">foundation_platform · ewe:// protocol</p>
</body></html>"#
    )
}

#[allow(dead_code)]
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod mode_page_tests {
    use super::*;

    #[test]
    fn build_mode_page_works_with_json_body() {
        let d = RouteDecision {
            source: RouteSource::WebviewApp,
            presentation: Presentation::Morph,
            view_kind: ViewKind::WebView,
            protocol: ProtocolHint::Default,
            cache_policy: CachePolicy::CacheFirst,
            profile: Profile::App,
            native_view_id: None,
            target: None,
            capabilities: vec![],
            auth_origin: None,
        };
        let (cls, label) = mode_badge_for(&d);
        let html = build_mode_page("/app/home", &(cls, label), &d, r#"{"type":"wasm_signal"}"#);
        assert!(html.contains("ewe:///app/home"));
        assert!(html.contains("wasm_signal"));
        assert!(html.contains("App"));
        assert!(!html.contains("panic"));
    }

    #[test]
    fn build_mode_page_escapes_html_in_body() {
        let d = RouteDecision {
            source: RouteSource::RemoteServer,
            presentation: Presentation::Morph,
            view_kind: ViewKind::WebView,
            protocol: ProtocolHint::Default,
            cache_policy: CachePolicy::NetworkFirst,
            profile: Profile::TrustedRemote,
            native_view_id: None,
            target: None,
            capabilities: vec![],
            auth_origin: None,
        };
        let (cls, label) = mode_badge_for(&d);
        let html = build_mode_page("/remote/data", &(cls, label), &d, "<script>alert('xss')</script>");
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>alert"));
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
