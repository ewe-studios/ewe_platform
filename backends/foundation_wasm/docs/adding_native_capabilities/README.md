# Adding Native Capabilities — foundation_wasm

`foundation_wasm` defines the **interface contract** for IPC capabilities — `Ipc`, `PlatformIpc`, `IpcRequest`, `IpcResponse`, `IpcError`. Implementing a new native capability starts here with the shared types, then the native handler goes in `foundation_platform_native`.

## The Ipc trait hierarchy

```rust
// Basic IPC — WASM-only, no session access
pub trait Ipc<Input = Vec<u8>, Output = Vec<u8>> {
    fn name(&self) -> &str;
    fn kind(&self) -> IpcKind;
    fn invoke(&self, request: &IpcRequest<Input>) -> Result<IpcResponse<Output>, IpcError>;
}

// Platform IPC — native handler with session access (in foundation_platform)
pub trait PlatformIpc {
    fn invoke_with_session(
        &self,
        request: &IpcRequest<Vec<u8>>,
        callback: IpcCallback,
    ) -> Result<(), IpcError>;
}
```

## WirePayload — serialization contract

Every data type crossing the IPC boundary implements `WirePayload`:

```rust
pub trait WirePayload: Sized {
    fn into_wire_bytes(self) -> (Vec<u8>, IpcContentType);
    fn from_wire_bytes(data: &[u8], ct: IpcContentType) -> Result<Self, WireError>;
}
```

This ensures both sides (WASM and native) agree on the wire format.

For the full step-by-step guide to adding a new native capability (shared types, native handler, WASM wrapper, Kotlin helper), see:
[`foundation_platform_native/docs/adding_native_capabilities/`](../../foundation_platform_native/docs/adding_native_capabilities/)
