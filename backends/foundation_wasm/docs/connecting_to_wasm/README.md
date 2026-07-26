# Connecting to WASM — foundation_wasm

`foundation_wasm` provides the **bridge infrastructure** — the FFI layer that connects WASM modules to native host code.

## The FFI boundary

Two exported functions cross the boundary:

| Direction | Function | Purpose |
|---|---|---|
| WASM → Host | `host_ipc_invoke(ptr, len, callback_id)` | WASM calls native IPC handler |
| WASM → Host | `host_ipc_invoke_async(ptr, len)` | WASM calls async native handler |
| Host → WASM | `ipc_resolve(slot_id, reply_bytes)` | Host resolves WASM async Promise |

## Memory sharing

WASM and the host share the same linear memory. `host_ipc_invoke` receives a pointer into WASM memory — the host reads the serialized request from there, processes it, and writes the response back via `ipc_resolve`.

```rust
// WASM side — writes request bytes to own memory, passes pointer to host
let ptr = mem::write_slice(&serialized_request);
host_ipc_invoke(ptr, serialized_request.len(), callback_id);

// Host side — reads from WASM memory, dispatches, writes result back
let request_bytes = wasm_memory.read(ptr, len);
let response = ipc_registry.dispatch(name, &request);
ipc_resolve(callback_id, &serialized_response);
```

## Trigger bridge

Triggers registered via `TriggerRegistry` are invoked by the host when platform events fire:

```rust
// Host fires a platform event
trigger_registry.fire("back_pressed", &[]);

// WASM modules subscribe to platform events
trigger_registry.register("back_pressed", |_| {
    // Handle back button press
});
```

## See also

- [`foundation_platform/docs/connecting_to_wasm/`](../../foundation_platform/docs/connecting_to_wasm/) — Session-side infrastructure
- [`foundation_platform_native/docs/connecting_to_wasm/`](../../foundation_platform_native/docs/connecting_to_wasm/) — Native capability patterns
