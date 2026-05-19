# foundation_http::wasm

Wasm modules for running `foundation_http` in browser and Cloudflare Workers environments.

## Architecture

```
wasm/
  bridge/     - JS interop bridges (web_sys::Request/Response, CF bindings)
  response.rs - WasmResponse struct and conversion helpers
  server.rs   - Request dispatch through middleware + router
  stream.rs   - Memory-backed stream collecting HTTP wire format
```

## Quick Start

### Cloudflare Workers

```rust
use foundation_http::wasm::bridge::cf::CfHttpApp;
use foundation_http::shared::method::Method;
use foundation_http::shared::router::HttpVerb;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = js_sys::Object)]
    type Env;
}

static APP: std::sync::OnceLock<CfHttpApp> = std::sync::OnceLock::new();

fn app() -> &'static CfHttpApp {
    APP.get_or_init(|| {
        let app = CfHttpApp::new();

        // Register routes on the inner HttpApp
        app.app().get("/api/health", |bag, req, writer| {
            use foundation_http::shared::serve::ServeWriter;
            writer.send_status(200).unwrap();
            writer.send_body(b"OK").unwrap();
            Ok(())
        });

        app
    })
}

#[wasm_bindgen]
pub async fn handle_request(req: web_sys::Request, env: Env) -> Result<web_sys::Response, wasm_bindgen::JsError> {
    app().handle_request(req, env.into()).await
}
```

### Browser / Generic wasm-bindgen

```rust
use foundation_http::wasm::bridge::web::WasmHttpApp;

static APP: std::sync::OnceLock<WasmHttpApp> = std::sync::OnceLock::new();

fn app() -> &'static WasmHttpApp {
    APP.get_or_init(|| {
        let app = WasmHttpApp::new();

        app.app().get("/hello", |bag, req, writer| {
            writer.send_body(b"Hello from wasm!").unwrap();
            Ok(())
        });

        app
    })
}

#[wasm_bindgen]
pub async fn handle_request(req: web_sys::Request) -> Result<web_sys::Response, wasm_bindgen::JsError> {
    app().handle_request(req).await
}
```

## Response Handling

Handlers write HTTP wire format through a `ServeWriter`. The `WasmStream` collects these bytes and parses them into a structured `WasmResponse`:

```rust
pub struct WasmResponse {
    pub status: u16,
    pub body: Option<Vec<u8>>,
    pub headers: Vec<(String, String)>,
}
```

### Setting status codes

```rust
// 404 Not Found
app.app().get("/missing", |_bag, _req, writer| {
    writer.send_status(404).unwrap();
    writer.send_body(b"Not found").unwrap();
    Ok(())
});

// 301 Redirect
app.app().get("/old-path", |_bag, _req, writer| {
    writer.send_status(301).unwrap();
    writer.send_header("Location", "https://example.com/new-path").unwrap();
    Ok(())
});

// 201 Created with JSON body
app.app().post("/api/items", |_bag, _req, writer| {
    writer.send_status(201).unwrap();
    writer.send_header("Content-Type", "application/json").unwrap();
    writer.send_body(br#"{"id": 1, "created": true}"#).unwrap();
    Ok(())
});
```

### Converting to web_sys::Response

```rust
use foundation_http::wasm::response::{from_wasm, from_bytes, build_response};

// From a WasmResponse
let wasm_resp = WasmResponse {
    status: 200,
    body: Some(b"hello".to_vec()),
    headers: vec![("Content-Type".into(), "text/plain".into())],
};
let response = from_wasm(wasm_resp)?;

// Directly from status + body
let response = from_bytes(200, b"hello")?;

// With explicit headers
let response = build_response(
    200,
    b"hello",
    vec![("Content-Type".into(), "text/plain".into())],
)?;
```

## Middleware

Middleware can short-circuit the request and return early:

```rust
use foundation_http::shared::middleware::Middleware;

struct AuthMiddleware;

impl Middleware for AuthMiddleware {
    fn handle(&self, bag: &ContextBag, req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        if !is_authenticated(req) {
            return MiddlewareResult::Response(SimpleOutgoingResponse {
                status: Status::UNAUTHORIZED,
                headers: SimpleHeaders::new(),
                body: Some(SendSafeBody::Text("Unauthorized".into())),
            });
        }
        MiddlewareResult::Continue
    }
}

// Register middleware
app.app().add_middleware(Arc::new(AuthMiddleware));
```

When middleware returns a `MiddlewareResult::Response`, the dispatcher converts it to a `WasmResponse` and returns it immediately without hitting the router.

## Cargo Features

| Feature | Description |
|---|---|
| `wasm-bindgen-http` | Enables `CfHttpApp` and `WasmHttpApp` bridges |

Dependencies (optional, enabled by `wasm-bindgen-http`):
- `wasm-bindgen`
- `wasm-bindgen-futures`
- `js-sys`
- `web-sys` (with `Headers`, `Request`, `Response`, `ResponseInit`)
