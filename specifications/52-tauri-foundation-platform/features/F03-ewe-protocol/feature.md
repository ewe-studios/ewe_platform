---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F03-ewe-protocol"
this_file: "specifications/52-tauri-foundation-platform/features/F03-ewe-protocol/feature.md"

status: pending
priority: critical
created: 2026-07-17

depends_on:
  - "F01-session-backbone"

tasks:
  completed: 0
  uncompleted: 18
  total: 18
  completion_percentage: 0%
---

# F03 — `ewe://` custom protocol and transport lanes

## Overview

Register `ewe://` as a single custom URI scheme with Tauri's
`UriSchemeProtocol`, implement protocol selection and content encoding,
wire all 7 transport lanes (pre-wired at session init, transport implied
by `RouteSource`). No sub-schemes — one scheme, transport from `RouteDecision`.

[Decision 03](../decisions/03-session-backbone-transport.md) sections 3-6
define the full transport architecture.

## Dependencies

Depends on: `F01-session-backbone`
Required by: `F09-walking-skeleton`

---

## Part A — `ewe://` registration

### A.1 — Single scheme registration

```rust
// foundation_platform/src/ewe.rs

use tauri::{
    http::{Request, Response, StatusCode},
    UriSchemeContext, UriSchemeResponder,
};

/// Register the `ewe://` custom protocol with Tauri.
/// Called from `PlatformBuilder::setup()`.
pub fn register_ewe_protocol<R: Runtime>(
    builder: tauri::Builder<R>,
) -> tauri::Builder<R> {
    builder.register_uri_scheme_protocol("ewe", ewe_protocol_handler)
}

/// The handler for every `ewe://` request.
/// Parse URI → resolve route → cache check → backend query →
/// protocol selection → encode → respond.
fn ewe_protocol_handler<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let session = ctx.app_handle.state::<PlatformSession<R>>();

    // 1. Parse URI
    let uri = request.uri().to_string();
    let url = match parse_ewe_url(&uri) {
        Ok(u) => u,
        Err(e) => {
            return responder.respond(
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(e.as_bytes().to_vec())
                    .unwrap(),
            );
        }
    };

    // 2. Resolve route through session handler chain
    let intent = NavigationIntent {
        url: uri.clone(),
        method: method_from_request(&request),
        source: IntentSource::LinkClick, // refined by caller
        referrer: None,
    };
    let decision = session.resolve_route(&intent);

    // 3. Cache check (step 3 of execution contract)
    let cache_key = url.path.clone();
    if session.cache().should_serve_cached(&decision, &cache_key) {
        if let Some(cached) = session.cache().get(&cache_key) {
            return responder.respond(cached.into_response());
        }
    }

    // 4. Backend query (step 5 — deferred to F09 walking skeleton)
    let content = session.query_backend(&decision, &url);

    // 5. Protocol selection (step 6)
    let protocol_hint = url.query_params().get("proto");
    let protocol = select_protocol(&decision.protocol, protocol_hint, &content);

    // 6. Encode response (step 7)
    let (body, content_type) = protocol.encode(&content);

    // 7. Respond
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type)
        .header("Access-Control-Allow-Origin", "ewe://localhost")
        .body(body)
        .unwrap();

    responder.respond(response);
}
```

### A.2 — URL parsing

```rust
struct EweUrl {
    path: String,
    query_params: HashMap<String, String>,
}

fn parse_ewe_url(uri: &str) -> Result<EweUrl, String> {
    // ewe://localhost/app/items?proto=arrow&cache=stale-while-revalidate
    let without_scheme = uri
        .strip_prefix("ewe://localhost")
        .ok_or("not an ewe:// URL")?;

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
```

---

## Part B — Protocol selection

### B.1 — Selection priority chain

```rust
/// Select the wire protocol for a response. Priority chain:
/// 1. `RouteDecision.protocol` — user's explicit choice
/// 2. `?proto=` query parameter — WebView requests a specific format
/// 3. Backend's native format — inferred from content bytes
/// 4. Platform default — columnar v1 for DomOps, Arrow binary for data
fn select_protocol(
    decision_hint: &ProtocolHint,
    query_hint: Option<&String>,
    content: &[u8],
) -> Protocol {
    // Priority 1: RouteDecision.protocol
    if decision_hint != &ProtocolHint::Default {
        return protocol_from_hint(decision_hint);
    }

    // Priority 2: ?proto= query parameter
    if let Some(hint) = query_hint {
        if let Some(p) = protocol_from_query(hint) {
            return p;
        }
    }

    // Priority 3: Detect from content
    if let Some(p) = detect_protocol(content) {
        return p;
    }

    // Priority 4: Platform default
    Protocol::Columnar
}

fn protocol_from_hint(hint: &ProtocolHint) -> Protocol {
    match hint {
        ProtocolHint::Default => Protocol::Columnar,
        ProtocolHint::Columnar => Protocol::Columnar,
        ProtocolHint::Arrow => Protocol::Arrow,
        ProtocolHint::ArrowIpc => Protocol::ArrowIpc,
        ProtocolHint::Json => Protocol::Json,
        ProtocolHint::Html => Protocol::Html,
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
    if content.is_empty() { return None; }
    // Arrow IPC stream: starts with 0xFFFFFFFF continuation + 0x00000000 schema
    if content.len() >= 8 && &content[..4] == b"\xFF\xFF\xFF\xFF" {
        return Some(Protocol::ArrowIpc);
    }
    // JSON: starts with '{' or '['
    if content[0] == b'{' || content[0] == b'[' {
        return Some(Protocol::Json);
    }
    // HTML: starts with '<'
    if content[0] == b'<' {
        return Some(Protocol::Html);
    }
    None
}
```

### B.2 — Protocol encoding

```rust
impl Protocol {
    /// Encode content bytes with the correct Content-Type header.
    /// For F03, this is pass-through encoding. Actual columnar/Arrow
    /// encoding logic lives in `foundation_wasm_ui` (imported later).
    pub fn encode(&self, content: &[u8]) -> (Vec<u8>, &'static str) {
        let ct = self.content_type();
        (content.to_vec(), ct)
    }
}
```

---

## Part C — Transport lanes

### C.1 — Transport enum

```rust
/// All 7 transport lanes from decision 03 section 4.
/// Pre-wired at session init. Transport selection is automatic from RouteSource.
pub enum Transport {
    /// ewe:// custom protocol — binary response with typed Content-Type.
    /// Registered once with Tauri's UriSchemeProtocol.
    CustomProtocol,

    /// tauri::command invoke — JSON for control, Raw(Vec<u8>) for Arrow binary.
    /// Used for IpcShell capability calls and control messages.
    CommandIpc,

    /// AppHandle::emit/listen — lifecycle, progress, invalidation notifications.
    /// Used for online/offline, sync_progress, cache_invalidated events.
    Events,

    /// Zero-copy Arrow via shared memory. Used for same-process native↔WASM.
    NativeShellIpc,

    /// HTTP/WebSocket to local embedded server.
    LocalServer,

    /// SSE/WebSocket to remote servers.
    RemoteSseWs,

    /// Standard HTTP fetch (ewe:// proxies to this for remote URLs).
    BrowserFetch,
}
```

### C.2 — Transport → RouteSource mapping

```rust
impl RouteSource {
    /// The transport lane automatically used for this source.
    /// This is NOT user-configurable — `RouteSource` implies the transport.
    pub fn default_transport(&self) -> Transport {
        match self {
            RouteSource::WebviewApp => Transport::CustomProtocol,
            RouteSource::IpcShell => Transport::CommandIpc,
            RouteSource::RemoteServer => Transport::CustomProtocol,
        }
    }
}
```

---

## Part D — Tauri command IPC (control lane)

### D.1 — `platform_invoke` command

```rust
/// Tauri command for capability invocations from the WebView.
/// Registered as `#[tauri::command]` in PlatformBuilder setup.
#[tauri::command]
fn platform_invoke<R: Runtime>(
    state: tauri::State<'_, PlatformSession<R>>,
    request: CapabilityRequest,
) -> Result<CapabilityResponse, String> {
    state.invoke_capability(&request)
}

impl<R: Runtime> PlatformSession<R> {
    /// Invoke a capability through the session backbone.
    /// Checks: profile gate → route allowlist → stale-page guard → execute.
    pub fn invoke_capability(
        &self,
        request: &CapabilityRequest,
    ) -> CapabilityResponse {
        // 1. Stale-page guard: is the requesting page still active?
        if !self.is_active_page(&request.page_identity) {
            return CapabilityResponse {
                id: request.id.clone(),
                page_identity: request.page_identity.clone(),
                status: Err("stale page".into()),
            };
        }

        // 2. Look up the capability handler
        let registry = self.capability_registry.read().unwrap();
        let handler = match registry.get(&request.capability) {
            Some(h) => h,
            None => return CapabilityResponse {
                id: request.id.clone(),
                page_identity: request.page_identity.clone(),
                status: Err(format!("unknown capability: {}", request.capability)),
            },
        };

        // 3. Execute (profile + per-route gates checked inside handler)
        let result = handler.execute(self, &request.action, request.payload.clone());
        CapabilityResponse {
            id: request.id.clone(),
            page_identity: request.page_identity.clone(),
            status: result,
        }
    }
}
```

---

## Verification

```bash
cargo test --package foundation_platform -- ewe
cargo test --package foundation_platform -- protocol
cargo test --package foundation_platform -- transport
```
