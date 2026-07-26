# Getting Started — foundation_platform

The EWE Platform coordination layer — session management, routing, IPC, WebView stack, and native capabilities, all built on Tauri v2.

## Setup

```toml
[dependencies]
foundation_platform = { path = "../../backends/foundation_platform" }
```

```rust
use foundation_platform::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    platform_run!(PlatformBuilder::new()
        .inject_platform_runtimes()
        .setup(setup_routes));
}

fn setup_routes(session: Arc<PlatformSession>) {
    // 1. Register WASM apps (from generated code)
    let app_responder = generated::app::AppAssets::build(&session);
    session.register_route_with("/app/*", webview_app().with_profile(Profile::App), app_responder);

    // 2. Register IPC handlers
    session.register_ipc(EchoIpc);

    // 3. Register capabilities
    session.register_capability(EchoCap);

    // 4. Register native modules
    foundation_platform_native::native::modal::register(Arc::clone(&session));
}
```

## Core concepts

### PlatformSession

The central coordination object — owned by a shared `Arc`. Holds:

- **Route table** — which responders handle which URL patterns
- **IpcRegistry** — named IPC handlers (`echo`, `system`, `chrome`, etc.)
- **CapabilityRegistry** — gated capability handlers with profile checks
- **WebViewStack** — navigation stack (push/pop/modal slots)
- **WindowManager** — window creation, pooling, destruction
- **AppHandle** — Tauri app handle for native operations

### Routes

Routes map URL patterns to responders. Each route has:
- A pattern (`/app/*`, `/api/invoke`)
- A `RouteDecision` (profile gate, presentation mode, responder type)
- A `RouteResponder` that produces `tauri::http::Response`

```rust
session.register_route_with(
    "/app/*",
    webview_app().with_profile(Profile::App),
    app_responder,
);

session.register_route_with(
    "/nav_modal",
    webview_app()
        .with_profile(Profile::App)
        .with_presentation(Presentation::Modal),
    ModeReportPage,
);
```

### IPC

Named IPC channels accessible from WASM and JavaScript:

```javascript
// JavaScript — direct from WebView
const result = await window.__TAURI_INTERNALS__.invoke('__ewe_ipc', {
    ipc: 'echo',
    action: 'ping',
    payload: new TextEncoder().encode(JSON.stringify({msg: 'hello'})),
    content_type: 'application/json'
});
```

### WebViewStack

Maintains the navigation stack (push, modal, morph, replace, root):

```rust
session.webview_stack().depth();           // current stack depth
session.webview_stack().active_route();    // currently visible route
session.webview_stack().push_slot(slot);   // push new slot
session.webview_stack().pop_with(|_,_|{}); // pop top slot
```

## PlatformBuilder

```rust
PlatformBuilder::new()
    .inject_platform_runtimes()     // WASM runtimes, HTTP backends
    .ota_manifest_domain("cdn.ewe.studio")   // OTA update domain
    .ota_manifest_key_from_str(key)          // OTA signing key
    .plugin(plugin())               // Tauri mobile plugins
    .setup(setup_routes)            // Route + IPC registration
```

`platform_run!` wraps the builder, creates the Tauri app, starts the session, and begins the event loop.
