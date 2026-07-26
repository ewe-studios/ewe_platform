# Deep Dives — foundation_platform

## Architecture layers

```
┌────────────────────────────────────────┐
│         foundation_platform            │
│  ┌──────────────────────────────────┐  │
│  │  Route Engine                    │  │
│  │  Pattern matching → Responder    │  │
│  │  Profile gating → Access decision│  │
│  └──────────────┬───────────────────┘  │
│  ┌──────────────▼───────────────────┐  │
│  │  IPC Registry                    │  │
│  │  name → Ipc handler lookup       │  │
│  │  Ipc + PlatformIpc dispatch      │  │
│  └──────────────┬───────────────────┘  │
│  ┌──────────────▼───────────────────┐  │
│  │  WebViewStack                    │  │
│  │  Slots: Push / Modal / Morph ... │  │
│  │  Pool management (create/destroy)│  │
│  └──────────────┬───────────────────┘  │
│  ┌──────────────▼───────────────────┐  │
│  │  WindowManager                   │  │
│  │  ensure / create / destroy       │  │
│  │  WebViewWrapper (Tauri window)   │  │
│  └──────────────────────────────────┘  │
│  ┌──────────────────────────────────┐  │
│  │  PlatformSession                 │  │
│  │  OTA updates, cache, overlays    │  │
│  │  Asset loading (VFS / APK)       │  │
│  └──────────────────────────────────┘  │
└────────────────────────────────────────┘
│               Tauri v2                  │
│  Window, WebView, Plugin system, IPC   │
└────────────────────────────────────────┘
```

## Routing pipeline

```
URL arrives (e.g. ewe://localhost/nav_modal)
  │
  ▼
Pattern matcher: which registered route matches?
  │  "/nav_modal" matches
  ▼
Profile check: is the source trusted enough?
  │  Profile::App → Access::Allowed
  ▼
Presentation: what navigation effect?
  │  Presentation::Modal → push modal slot
  ▼
WebViewStack: create or reuse a WebView?
  │  new slot → WindowManager.ensure() → Tauri WebviewWindowBuilder
  ▼
Responder: produce the HTTP response
  │  generates HTML, returns tauri::http::Response
  ▼
WebView displays the page
```

## IPC dispatch flow

```
WASM/JS
  │
  invoke('__ewe_ipc', {ipc, action, payload, content_type})
  │
  ▼  Tauri IPC bridge
  │
  ▼
IpcRegistry::dispatch(name, request)
  │
  ├─ Query/Ipc handler → Ipc::invoke(&request) → Ok(response)
  │
  └─ PlatformIpc handler → 
       PlatformIpc::invoke_with_session(&request, &session, callback)
         │
         └─ Handler has full session access:
            ├─ session.webview_stack() — navigation state
            ├─ session.window_manager() — create/destroy windows
            ├─ session.handles::<AppHandle>() — Tauri native APIs
            └─ callback(Ok(response)) — resolve WASM Promise
```

## WebViewStack slot types

| Presentation | Behavior |
|---|---|
| **Push** | New WebView on top, screenshot old one |
| **Modal** | New WebView as overlay (SheetWryActivity on Android) |
| **Morph** | In-place DOM update (reuse current WebView) |
| **Replace** | Swap current slot's route |
| **Root** | Clear all slots, start fresh |
| **External** | Open in system browser |

```rust
pub enum Presentation {
    Push,
    Modal,
    Morph,
    Replace,
    Root,
    External,  // system browser
}
```

## WindowManager pooling

```
WindowManager::ensure(pool, route)
  │
  ├─ Pool has an idle WebView for this route? → reuse it
  │
  ├─ Pool can create a new WebView? → WebviewWindowBuilder::new()
  │   │
  │   └─ Android: activity_name("SheetWryActivity")
  │      Desktop: standard window with decorations(false)
  │
  └─ Pool is full? → evict oldest idle WebView, create new one
```

## Session transport

```
SessionTransport
  │
  ├─ PlatformSession → IPC handlers
  │   ├─ BackendTransport (HTTP fetch via session.http_backend())
  │   └─ dispatch_ipc (named IPC channels)
  │
  ├─ ClosureTransport (closure-based transport for inline responders)
  │
  └─ DefaultTransport (session-aware, route-aware)
```
