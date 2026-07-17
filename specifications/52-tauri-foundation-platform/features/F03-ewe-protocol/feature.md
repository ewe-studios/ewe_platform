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
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---

# F03 — `ewe://` custom protocol and transport lanes

## Overview

Register `ewe://` as a custom URI scheme with Tauri's `UriSchemeProtocol`,
wire the transport lanes (command IPC, custom protocol binary, events), and
implement protocol selection and content encoding for responses.

[Decision 03](../decisions/03-session-backbone-transport.md) defines the full
transport and protocol architecture. Single `ewe://` scheme — no sub-schemes.

## Dependencies

Depends on:
- `F01-session-backbone` — Protocol handler routes through session backbone

Required by:
- `F08-walking-skeleton` — End-to-end request-to-render flow

## Requirements

### 1. ewe:// custom protocol registration

Register a single `ewe://` scheme with Tauri:

```rust
// In PlatformBuilder setup:
impl PlatformBuilder {
    pub fn register_ewe_protocol(self) -> Self {
        self.inner.register_uri_scheme_protocol("ewe", move |ctx, request, responder| {
            // Parse URI, route through session backbone
            let url = request.uri().parse::<Url>()?;
            let protocol_hint = extract_query_param(&url, "proto");

            // Session resolves the route, selects protocol, encodes response
            let response = session.handle_ewe_request(&url, protocol_hint);

            responder.respond(response);
        });
        self
    }
}
```

### 2. Session request handler

```rust
impl<R: Runtime> PlatformSession<R> {
    fn handle_ewe_request(&self, url: &Url, protocol_hint: Option<&str>) -> Response<Vec<u8>> {
        // 1. Resolve route
        let intent = NavigationIntent::from_url(url);
        let decision = self.resolve_route(&intent);

        // 2. Check cache
        if self.cache.should_serve_cached(&decision, url) {
            return self.cache.get(url).unwrap().into_response();
        }

        // 3. Query backend
        let content = self.query_backend(&decision, url);

        // 4. Select protocol and encode
        let protocol = Protocol::from_hint(&decision.protocol, protocol_hint);
        let (body, content_type) = protocol.encode(&content);

        // 5. Build HTTP response
        Response::builder()
            .status(200)
            .header("Content-Type", content_type)
            .header("Access-Control-Allow-Origin", "ewe://localhost")
            .body(body)
            .unwrap()
    }
}
```

### 3. Transport lanes

All 7 lanes from [decision 03](../decisions/03-session-backbone-transport.md),
pre-wired at session init:

```rust
pub enum Transport {
    CustomProtocol,    // ewe:// — binary response with typed Content-Type
    CommandIpc,        // tauri::command invoke — JSON + Raw(Vec<u8>)
    Events,            // AppHandle::emit/listen — lifecycle notifications
    NativeShellIpc,    // Zero-copy Arrow via shared memory
    LocalServer,       // HTTP/WS to local embedded server
    RemoteSseWs,       // SSE/WebSocket to remote
    BrowserFetch,      // Standard HTTP fetch
}
```

Transport selection is automatic from `RouteSource` — user never picks transport.

### 4. Protocol encoding

```rust
impl Protocol {
    pub fn encode(&self, content: &[u8]) -> (Vec<u8>, &'static str) {
        match self {
            Protocol::Columnar => (content.to_vec(), "application/primal-columnar"),
            Protocol::Arrow => (content.to_vec(), "application/primal-arrow"),
            Protocol::ArrowIpc => (content.to_vec(), "application/vnd.apache.arrow.stream"),
            Protocol::Json => (content.to_vec(), "application/primal-json"),
            Protocol::Html => (content.to_vec(), "text/html; charset=utf-8"),
            Protocol::CustomBinary => (content.to_vec(), "application/primal-binary"),
        }
    }

    pub fn from_hint(hint: &ProtocolHint, query_hint: Option<&str>) -> Self {
        // Priority:
        // 1. RouteDecision.protocol
        // 2. ?proto= query parameter
        // 3. Default: columnar for DomOps, Arrow for data
    }
}
```

### 5. URI structure

```
ewe://localhost/{route}?proto=arrow&cache=stale-while-revalidate&action=write
```

Single scheme, no sub-schemes. Transport implied by `RouteSource`.

### 6. Tauri IPC lane for capabilities

```rust
// Capability calls go through tauri::command invoke:
#[tauri::command]
fn platform_invoke(request: CapabilityRequest) -> CapabilityResponse {
    session.route_capability(request)
}
```

## Tasks

### Custom protocol
- [ ] Register `ewe://` scheme in PlatformBuilder via `register_uri_scheme_protocol`
- [ ] Implement `handle_ewe_request()` — URL parse → resolve → cache check → backend → encode → respond
- [ ] Test: ewe:// URL intercepted and routed through session

### Transport lanes
- [ ] Define `Transport` enum with 7 variants
- [ ] Pre-wire all lanes at `PlatformSession::initialize()`
- [ ] Implement transport selection from RouteSource

### Protocol encoding
- [ ] Implement `Protocol::encode()` for all 6 variants
- [ ] Implement `Protocol::from_hint()` with priority chain
- [ ] Test: Columnar → `application/primal-columnar` Content-Type
- [ ] Test: Arrow → `application/primal-arrow` Content-Type
- [ ] Test: JSON → `application/primal-json` Content-Type

### Tauri command IPC
- [ ] Define `platform_invoke` tauri command
- [ ] Route capability requests through session's capability registry
- [ ] Test: capability request dispatched and response returned

## Verification Commands

```bash
cargo test --package foundation_platform -- ewe
cargo test --package foundation_platform -- protocol
cargo test --package foundation_platform -- transport
```
