# Deep Dives — foundation_wasm

## IPC FFI bridge

The bridge connects WASM (compiled to wasm32) with the native host binary:

```
WASM module                          Native host
───────────                          ───────────
ipc_dispatch(name, action, bytes)
  │
  ▼ FFI: host_ipc_invoke(ptr, len, cb_id)
  │                                       │
  │                                  IpcRegistry::dispatch()
  │                                       │
  │                                  handler.invoke(request)
  │                                       │
  ▼ FFI: ipc_resolve(cb_id, result)  ◄────┘
  │
  ▼ Promise<Result<IpcResponse, IpcError>>
```

### Async: `host_ipc_invoke_async` + `ipc_resolve`

Native handlers that implement `PlatformIpc::invoke_with_session` use a callback pattern:

1. WASM calls `host_ipc_invoke_async(ptr, len)` — returns an `IpcSlot` ID
2. Native handler processes the request asynchronously
3. Native handler calls `callback(Ok(response))` when done
4. Callback invokes `ipc_resolve(slot_id, reply_bytes)` on the WASM side
5. WASM Promise resolves with the decoded response

### ReplyEncoder internals

```
Reply format:
  byte 0:   0x64 (100 decimal) = Begin marker
  byte 1:   0x12 = IpcSlot (callback slot ID follows)
  bytes 2-5: u32 LE slot_id
  byte 6:   0x65 (101 decimal) = End marker

Error reply:
  byte 0:   0x64 (100) = Begin
  byte 1:   0x11 = IpcError
  bytes 2-5: u32 LE error_code
  byte 6:   0x65 (101) = End
```

## IpcRegistry

Central dispatch table for named IPC handlers:

```rust
pub struct IpcRegistry {
    ipcs: HashMap<String, Box<dyn AnyIpc>>,
}

impl IpcRegistry {
    pub fn register<I: Ipc + 'static>(&mut self, handler: I);
    pub fn dispatch(&self, name: &str, request: &IpcRequest<Vec<u8>>)
        -> Result<IpcResponse<Vec<u8>>, IpcError>;
    pub fn names(&self) -> Vec<String>;  // for introspection (e.g. /api/system page)
}
```

Lookup is by `Ipc::name()` — the `"chrome"` in `invoke('__ewe_ipc', {ipc: 'chrome', ...})`.

## IpcKind variants

| Kind | Purpose |
|---|---|
| `Query` | Read-only data fetch — `echo`, `system`, `ticker` |
| `Command` | State-changing operation |
| `Capability` | Security-gated operation — requires profile check |

## WASM memory model

`foundation_wasm` provides `WasmMemory` — a safe abstraction over the WASM linear memory:

```rust
pub struct WasmMemory {
    memory: Memory,        // WebAssembly.Memory handle
    alloc_fn: fn(u32) -> u32,
    dealloc_fn: fn(u32, u32),
}
```

`WasmMemory::create(data, runtime)` allocates in WASM heap, writes bytes, and returns a pointer. `WasmMemory::read(ptr, len)` reads bytes back. This is the plumbing that `host_ipc_invoke` uses to pass serialized requests across the boundary.

## Trigger Registry

Event system for module-to-module communication within a WASM app:

- Each trigger is a named slot with a callback
- `TriggerRegistry::fire(name, payload)` invokes all registered callbacks for that name
- Used for: navigation events, modal lifecycle, signal delivery

## Build tools

`build_tools/` provides `WasmBundleGenerator` — a build-time tool that:
1. Scans the WASM output directory
2. Collects `.wasm` + `.js` files
3. Generates a `.ewe_manifest.json` with version + hash
4. Produces a deployable bundle ready for APK embedding
