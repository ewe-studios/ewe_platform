# Connecting to WASM — foundation_platform

`foundation_platform` provides the session-side infrastructure that WASM apps connect to.

## IPC from WASM

The `SessionTransport` bridges WASM IPC calls to the session's `IpcRegistry`:

```rust
// WASM side — calls ipc_dispatch which routes through SessionTransport
let response: IpcResponse<Vec<u8>> = dispatch_ipc(request).await?;
```

The session wires this up automatically during `platform_run!`:

```rust
platform_run!(PlatformBuilder::new()
    .inject_platform_runtimes()   // ← creates SessionTransport, HTTP backends
    .setup(setup_routes)
);
```

## Registration patterns

### Query/IPC handlers (no session needed)

```rust
struct EchoIpc;
impl Ipc for EchoIpc {
    fn invoke(&self, request: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError>
}
session.register_ipc(EchoIpc);
```

### Platform handlers (session needed)

```rust
impl PlatformIpc for ModalIpc {
    fn invoke_with_session(&self, request, callback) -> Result<(), IpcError> {
        // Has access to: self.session.webview_stack(),
        //                 self.session.window_manager(),
        //                 self.session.handles::<AppHandle>()
    }
}
session.register_ipc(modal_ipc);
```

### Capability handlers (gated)

```rust
impl PlatformIpc for EchoCap {
    fn capability_id(&self) -> &CapabilityId;
    fn min_profile(&self) -> Profile { Profile::TrustedRemote }
}
session.register_capability(EchoCap);
```

## WASM module loading

The session can serve WASM apps through multiple backends:

- **Asset responders** — serve `.wasm` + `.js` from bundled assets (APK, VFS)
- **Wasmtime responders** — `wasmtime_app()` runs WASM in wasmtime (desktop)
- **WebView responders** — `webview_app()` loads WASM in a Tauri WebView

```rust
// Serve a WASM app via Tauri WebView
session.register_route_with(
    "/app/*",
    webview_app().with_profile(Profile::App),
    generated::app::AppAssets::build(&session),
);
```

## The Tauri IPC bridge

Foundation-platform WASM apps use Tauri's built-in IPC:

```javascript
// JavaScript → Rust IPC
const result = await window.__TAURI_INTERNALS__.invoke('__ewe_ipc', {
    ipc: 'system',
    action: 'get_info',
    payload: [],
    content_type: 'application/json'
});
```

The `'__ewe_ipc'` command is registered by the platform. It dispatches through `IpcRegistry::dispatch()`, passing the `ipc` name to find the right handler and `action` to select the method.
