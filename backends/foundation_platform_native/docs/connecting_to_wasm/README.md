# Connecting to WASM — foundation_platform_native

How WASM apps call native capabilities through the IPC bridge.

## The bridge

WASM apps don't link against the native binary. They communicate via a serialized IPC channel:

```
WASM wasm32-unknown-unknown          Native aarch64/x86_64
───────────────────────────          ─────────────────────
  ipc_dispatch(name, action, bytes)
      │                                        │
      ▼ FFI call                               ▼
  host_ipc_invoke(ptr, len, cb_id)  →  IpcRegistry::dispatch()
                                              │
      ◄── ipc_resolve(cb_id, result)  ──  callback(Ok(response))
      │
  Promise<Result<IpcResponse, IpcError>>
```

## Using a capability from WASM

```rust
use foundation_platform_native::shared::modal_types::PresentArgs;
use foundation_platform_native::wasm::modal::Modal;

// Present a bottom sheet modal
let result = Modal::present(PresentArgs {
    route: "/app/settings".into(),
    style: Some("bottom_sheet".into()),
    title: Some("Settings".into()),
}).await;
match result {
    Ok(res) => log::info!("Modal created: {}", res.modal_id),
    Err(e) => log::error!("Modal failed: {:?}", e),
}

// Dismiss
Modal::dismiss(&res.modal_id).await?;
```

## The ReplyEncoder format

IPC responses use the `ReplyEncoder` binary encoding:

```
[100:Begin][ReturnType:u8][payload bytes...][101:End]
```

Where `ReturnType` is:
- `0x00` = Void
- `0x01` = JSON (serde-encoded)
- `0x02` = Binary (raw bytes)
- `0x11` = Error (IpcError code follows)

The WASM side decodes this via `ReplyDecoder`, yielding `Result<IpcResponse, IpcError>`.

## Dispatching from raw WASM (no wrappers)

If you prefer to call the IPC bridge directly from JavaScript/WASM without Rust wrappers:

```javascript
// JavaScript — direct IPC call
const result = await window.__TAURI_INTERNALS__.invoke('__ewe_ipc', {
    ipc: 'chrome',
    action: 'present_modal',
    payload: new TextEncoder().encode(JSON.stringify({
        url: 'http://ewe.localhost/app/settings',
        style: 'bottom_sheet',
        title: 'Settings'
    })),
    content_type: 'application/json'
});
```

## Registering on the session

Each native IPC handler must be registered during app startup:

```rust
fn setup_routes(session: Arc<PlatformSession>) {
    foundation_platform_native::native::modal::register(Arc::clone(&session));
    foundation_platform_native::native::dialog::register(Arc::clone(&session));
}
```

The `register()` function calls `session.register_ipc(handler)` which adds the handler to the `IpcRegistry`. When a WASM app dispatches `chrome → present_modal`, the registry looks up `chrome` and routes to the matching handler.

## Error handling

| IpcError variant | When |
|---|---|
| `InvalidPayload` | WASM sent malformed JSON that doesn't deserialize into the expected type |
| `ExecutionFailed` | Native handler couldn't complete the operation (e.g. window creation failed) |
| `NotFound` | No handler registered for the requested IPC name |
| `Unauthorized` | Capability gate rejected the request (profile/permission check) |

WASM callers should always handle `ExecutionFailed` — it's the catch-all for native failures:
```rust
match Modal::present(args).await {
    Ok(result) => { /* modal created */ }
    Err(IpcError::ExecutionFailed) => { /* native side couldn't create the window */ }
    Err(IpcError::InvalidPayload) => { /* bad args — fix the caller */ }
    Err(e) => { /* unexpected */ }
}
```
