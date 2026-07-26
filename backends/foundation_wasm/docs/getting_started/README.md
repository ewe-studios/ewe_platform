# Getting Started — foundation_wasm

Core WASM IPC, FFI bridge, and type system for EWE Platform apps. Provides the `Ipc` trait, `IpcRequest`/`IpcResponse`, `ReplyEncoder`, trigger registry, and the WASI host runtime.

## Key modules

| Module | Purpose |
|---|---|
| `ipc` | `Ipc` trait, `IpcRequest<I>`, `IpcResponse<O>`, `IpcError`, `IpcKind` |
| `ipc_ffi` | FFI bridge: `host_ipc_invoke` (WASM→host) and `ipc_resolve` (host→WASM) |
| `reply_encoder` | Binary encoding format for IPC responses across FFI boundary |
| `trigger` | `TriggerRegistry` — event dispatch system |
| `host_runtime` | WASI host runtime, memory management, JS bridge bindings |

## Ipc trait

```rust
pub trait Ipc<Input = Vec<u8>, Output = Vec<u8>> {
    fn name(&self) -> &str;
    fn kind(&self) -> IpcKind;  // Query, Command, Capability
    fn invoke(&self, request: &IpcRequest<Input>) -> Result<IpcResponse<Output>, IpcError>;
}
```

### Typed requests

```rust
#[derive(serde::Serialize, serde::Deserialize)]
struct PresentArgs { route: String, style: Option<String>, title: Option<String> }

let typed: IpcRequest<PresentArgs> = raw_request.clone().into_typed()?;
// typed.payload is a PresentArgs struct
```

## ReplyEncoder format

IPC responses across the FFI boundary use a binary format:

```
[100:Begin][ReturnType:u8][payload bytes...][101:End]
```

| ReturnType | Meaning |
|---|---|
| `0x00` | Void (no return value) |
| `0x01` | JSON (serde-encoded struct) |
| `0x02` | Binary (raw bytes) |
| `0x12` | IpcSlot — async callback slot ID |
| `0x11` | IpcError — error code follows |

## Trigger Registry

Events dispatched between WASM modules:

```rust
// Register a trigger
trigger_registry.register("modal_dismissed", |payload: &[u8]| {
    // handle the event
});

// Fire a trigger
trigger_registry.fire("modal_dismissed", &payload_bytes);
```

## Platform scheme interceptor

`PLATFORM_SCHEME_INTERCEPTOR_JS` is embedded JavaScript that intercepts `ewe://localhost/...` links in WebViews and routes them through the platform's navigation system instead of letting the system browser handle them.
