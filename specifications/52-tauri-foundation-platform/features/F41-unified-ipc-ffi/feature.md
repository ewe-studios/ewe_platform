---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F41-unified-ipc-ffi"
this_file: "specifications/52-tauri-foundation-platform/features/F41-unified-ipc-ffi/feature.md"

status: in-progress
priority: critical
created: 2026-07-24
updated: 2026-07-24

depends_on:
  - "F23-wasm-capabilities"
  - "F25-ipc-registry"
  - "F28-wasm-stream-registry"

tasks:
  completed: 6
  uncompleted: 3
  total: 9
  completion_percentage: 67%
---
# F41 — Unified IPC: merge Capability, add FFI, platform handles, typed WASM API

## Problem

Four gaps. They all trace back to the same root: IPC and Capability are separate,
neither has FFI, and the WASM side has no typed API.

### Gap 1 — IPC and Capability are the same thing

`foundation_wasm::ipc::Ipc` and `foundation_wasm::WasmCapability` are identical
in shape: `name()`, `invoke(Request<Vec<u8>>) -> Response<Vec<u8>>`, `Box<dyn>`
registry. The differences (`action` placement, `IpcError` vs `CapabilityError`)
don't justify two traits, two registries, two Tauri commands (`__ewe_ipc` +
`__ewe_capabilities`), and two JS bridges.

**Every capability IS an IPC.** Camera capture = `IpcRequest { ipc: "camera",
action: "capture", payload: ... }`.

### Gap 2 — No FFI bindings on either

`host_runtime.rs` has `#[link(wasm_import_module = "abi")] extern "C" { ... }`.
`stream.rs` has WASM exports for `stream_create/send/close`. But `Ipc` and
`WasmCapability` have **no FFI at all.** They invoke through Tauri's
`__TAURI_INTERNALS__.invoke()` — a Tauri-specific transport. No `extern "C"`
import for WASM to call, no export for the host to push events back. Non-Tauri
hosts (Deno, browser) can't implement IPC.

### Gap 3 — No calling context for handlers

`PlatformIpc::invoke_with_session` gets `&PlatformSession` but has no idea WHICH
page/window invoked it. A camera handler that launches a native intent needs to
know which `Activity` to use. A streaming handler returning frames needs to know
where to push them. Tauri's `Invoke` already has `window_label()` — we never
capture it.

### Gap 4 — No typed WASM-side API

WASM apps call `invoke('__ewe_ipc', { ipc: "camera", action: "capture" })` with
raw JSON. There's no `Camera::open()`, no `Biometric::authenticate()`. Each app
writes its own serialization around the raw invoke call.

## Solution

Four parts, matching the four gaps:

### Part A — Merge `WasmCapability` into `Ipc` (gap 1)

Delete `WasmCapability`, `CapabilityRequest`, `CapabilityResponse`,
`CapabilityError`, `CapabilityContentType`, `CapabilityRegistry` from
`foundation_wasm`. Everything folds into the existing `Ipc` infrastructure.

```rust
// foundation_wasm/src/ipc.rs

pub trait Ipc<Input = Vec<u8>, Output = Vec<u8>> {
    fn name(&self) -> &str;
    fn kind(&self) -> IpcKind;
    fn invoke(
        &self,
        request: &IpcRequest<Input>,
    ) -> Result<IpcResponse<Output>, IpcError>;
}
```

`IpcKind` gains the capability variant with inline security metadata:

```rust
pub enum IpcKind {
    Query,
    Emit,
    Page,
    Capability { min_profile: Profile },
}
```

`IpcError` absorbs `CapabilityError`'s `PermissionDenied`:

```rust
pub enum IpcError {
    UnknownIpc(String),
    InvalidPayload(String),
    ExecutionFailed(String),
    PermissionDenied(String),
    DomainError { code: String, message: String },
}
```

`IpcContentType` stays — it's already a type alias for `CapabilityContentType`.
The single `IpcRegistry` in `foundation_wasm` stores `Box<dyn Ipc<Vec<u8>,
Vec<u8>>>`.

### The Output type IS the delivery model

Because `Ipc<Input, Output>` is generic over both parameters, the `Output` type
declares the delivery contract. No separate enums. No `CapabilityOutcome`. The
type system encodes it:

| Output type | Delivery | Example |
|---|---|---|
| `Vec<u8>` | Wire bytes, sync | Echo, system info |
| `SomeStruct` | Typed data, sync | `CameraHandle { handle: 7 }`, `PhotoResult { uri }` |
| `u64` (stream ID) | Streaming — handler creates a `HostStreamQueue`, WASM polls chunks | Camera frame stream, sensor data |
| `()` | Fire-and-forget | Toolbar button added, share sheet dismissed |
| `Pin<Box<dyn Future<Output = Result<T>>>>` | Async — caller drives the future | Long-running biometric prompt |

The **capability author** chooses the output type when they write the handler.
The **WASM wrapper** in `foundation_platform/src/wasm/` presents the matching
typed API. The **host** just moves bytes — it never branches on delivery mode.

```rust
// Sync data response
impl Ipc<CameraOpenArgs, CameraHandle> for NativeCamera { ... }

// Stream response — Output is StreamId, WASM side polls the stream
impl Ipc<CameraSnapArgs, StreamId> for NativeCamera { ... }

// Fire-and-forget — Output is (), no response expected
impl Ipc<ToolbarButtonArgs, ()> for NativeChrome { ... }
```

The WASM wrapper for each exposes the natural calling convention:

```rust
// foundation_platform/src/wasm/camera.rs

impl Camera {
    /// Sync: block until the camera is open, returns handle.
    pub fn open(args: CameraOpenArgs) -> Result<CameraHandle, IpcError> { ... }

    /// Stream: returns a StreamId. Caller reads frames from the stream.
    pub fn snap(args: CameraSnapArgs) -> Result<StreamId, IpcError> { ... }
}
```

### Capabilities are written in foundation_platform, properly per target

Each capability gets one implementation per target that `foundation_platform`
supports. The registry picks the right one at dispatch time:

```
foundation_platform/src/
├── ipc/
│   ├── camera.rs         ← struct NativeCamera;
│   │                        impl Ipc for NativeCamera { ... }            (pure, desktop fallback)
│   │                        #[cfg(target_os = "android")]
│   │                        impl AndroidIpc for NativeCamera { ... }     (JNI camera intent)
│   │                        #[cfg(target_os = "ios")]
│   │                        impl IosIpc for NativeCamera { ... }         (AVCaptureSession)
│   ├── filesystem.rs     ← struct FilePicker; same pattern
│   ├── biometric.rs      ← struct BiometricAuth; same pattern
│   ├── chrome.rs         ← struct NativeChrome; same pattern
│   └── mod.rs            ← registers all built-in capability handlers
│
├── wasm/                  ← #[cfg(target_family = "wasm")]
│   ├── camera.rs         ← struct Camera; typed wrapper over host_ipc_dispatch
│   ├── filesystem.rs     ← struct FilePicker;
│   ├── biometric.rs      ← struct BiometricAuth;
│   ├── chrome.rs         ← struct Chrome;
│   └── mod.rs            ← re-exports
│
├── handle.rs             ← platform handles, traits, registries
└── session.rs            ← two-tier dispatch
```

Each target handles delivery differently:

| Target | Delivery mechanism |
|---|---|
| **Android** | `AndroidIpc::invoke_android` → JNI → Android SDK APIs. Response serialized to wire bytes. |
| **iOS** | `IosIpc::invoke_ios` → Swift FFI → iOS SDK APIs. Response serialized to wire bytes. |
| **Desktop** | `Ipc::invoke` → Tauri plugin or pure Rust. Response serialized to wire bytes. |
| **Web (wasm)** | `host_ipc_invoke(ptr, len)` → host JS bridge. Browser host returns `ExecutionFailed("unsupported")` for capabilities with no web API equivalent, or delegates to Web APIs (e.g., `navigator.mediaDevices.getUserMedia` for camera). |

The host's only job is to deserialize the request, dispatch to the right handler,
serialize the response. Delivery mode (sync/async/stream) is between the handler
and the WASM wrapper — the host just moves bytes.

### Part B — Define the WASM↔Host FFI (gap 2)

Same pattern as `host_runtime.rs`'s `#[link(wasm_import_module = "abi")]`.

**WASM imports** — the WASM side calls these, every host implements them:

```rust
// foundation_wasm/src/ipc.rs, #[cfg(target_family = "wasm")]

#[link(wasm_import_module = "platform")]
extern "C" {
    /// Invoke a named IPC handler on the host. Returns an allocation ID.
    /// 0 = error. Non-zero = allocation containing the serialized
    /// IpcResponse bytes. Caller reads from allocation, then calls
    /// dispose_allocation().
    pub fn host_ipc_invoke(
        request_ptr: *const u8,
        request_len: u32,
    ) -> u64;

    /// Open a host→WASM stream for this IPC. Returns a stream ID.
    /// 0 = error. WASM polls chunks via host_stream_read(id).
    pub fn host_ipc_stream_open(
        request_ptr: *const u8,
        request_len: u32,
    ) -> u64;

    /// Close a host→WASM stream.
    pub fn host_ipc_stream_close(stream_id: u64);
}
```

**WASM export** — the host calls this to push events back:

```rust
// foundation_wasm/src/ipc.rs

/// Host calls this to push an event to the WASM app.
/// Returns 0 on failure, non-zero on success.
#[no_mangle]
pub extern "C" fn ipc_handle_event(
    event_ptr: *const u8,
    event_len: u32,
) -> u64;
```

**Typed dispatch helper** — hides the FFI plumbing behind `WirePayload`:

```rust
// foundation_wasm/src/ipc.rs

/// Serialize → host_ipc_invoke → deserialize. Same for ALL IPCs.
pub fn host_ipc_dispatch<T: WirePayload, U: WirePayload>(
    name: &str,
    request: &IpcRequest<T>,
) -> Result<IpcResponse<U>, IpcError> {
    let wire_req = IpcRequest {
        ipc: name.to_string(),
        action: request.action.clone(),
        payload: request.payload.into_wire_bytes().0,
        content_type: request.content_type,
        target: request.target.clone(),
    };
    // serialize to bytes, call host_ipc_invoke, read allocation, deserialize
    ...
}
```

The full round-trip:

```
WASM side                                    Host side
─────────                                    ─────────
host_ipc_dispatch::<Args, Result>("camera", req)
  → serialize IpcRequest<Args> to bytes
  → host_ipc_invoke(ptr, len)
                                              → deserialize bytes → IpcRequest<Vec<u8>>
                                              → invoke_ipc(request, calling_page)
                                              → handler.invoke_android(...) or .invoke(...)
                                              → serialize IpcResponse
                                              → allocate response bytes
                                              → return allocation_id
  ← allocation_id
  → read response from allocation
  → dispose_allocation(allocation_id)
  → deserialize → IpcResponse<Result>
  → Ok(result)
```

### Part C — Platform handles, context, and host-stream registry (gaps 2+3)

#### C1. IpcInvokeContext — who called this?

```rust
// foundation_platform/src/handle.rs

/// Passed to every IPC handler at invoke time.
/// Carries the calling page identity and the platform handle.
pub struct IpcInvokeContext {
    /// Which page invoked this IPC.
    pub page: PageIdentity,
    /// The WebView label for scoping emits and stream delivery.
    pub webview_label: String,
    /// The session.
    pub session: Arc<PlatformSession>,
}
```

The Tauri command captures `window.label()` at invoke time and wraps it:

```rust
#[tauri::command]
fn __ewe_ipc(
    window: tauri::Window,
    session: tauri::State<'_, Arc<PlatformSession>>,
    ipc: String, action: String, payload: Option<Vec<u8>>, ...
) -> Result<Vec<u8>, String> {
    let ctx = IpcInvokeContext {
        page: session.active_page_identity().unwrap_or_default(),
        webview_label: window.label().to_string(),
        session: session.inner().clone(),
    };
    session.invoke_ipc(&ctx, &request)
}
```

#### C2. Platform handles

```rust
// foundation_platform/src/handle.rs

#[cfg(target_os = "android")]
pub struct AndroidHandle {
    pub jni_env: *mut std::ffi::c_void,   // JNIEnv*
    pub activity: *mut std::ffi::c_void,  // jobject (Activity)
    pub webview: *mut std::ffi::c_void,   // jobject (WebView)
}

#[cfg(target_os = "ios")]
pub struct IosHandle {
    pub view_controller: *mut std::ffi::c_void,  // UIViewController*
    pub window: *mut std::ffi::c_void,            // UIWindow*
    pub webview: *mut std::ffi::c_void,           // WKWebView*
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub struct DesktopHandle {
    pub window_label: String,
}
```

Inherent methods for common OS operations — `start_activity_for_result`,
`request_permission`, `present_view_controller`. Not on a trait; only available
when compiling for that target.

#### C3. Platform dispatch traits

```rust
// foundation_platform/src/handle.rs

#[cfg(target_os = "android")]
pub trait AndroidIpc: foundation_wasm::ipc::Ipc {
    fn invoke_android(
        &self,
        ctx: &IpcInvokeContext,
        handle: &AndroidHandle,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError>;
}

#[cfg(target_os = "ios")]
pub trait IosIpc: foundation_wasm::ipc::Ipc {
    fn invoke_ios(
        &self,
        ctx: &IpcInvokeContext,
        handle: &IosHandle,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError>;
}

// Desktop: no separate trait needed. Ipc::invoke is pure Rust.
```

#### C4. CapabilityHandleRegistry

Same pattern as `StreamRegistry` (F28) — opaque `u64` tokens → native resources:

```rust
// foundation_platform/src/handle.rs

pub struct CapabilityHandleRegistry {
    next_id: Mutex<u64>,
    handles: Mutex<BTreeMap<u64, Box<dyn Any + Send>>>,
}

impl CapabilityHandleRegistry {
    pub fn insert<T: Send + 'static>(&self, resource: T) -> u64 { ... }
    pub fn get_mut<T: 'static>(&self, id: u64) -> Option<&mut T> { ... }
    pub fn remove(&self, id: u64) -> Option<Box<dyn Any + Send>> { ... }
}
```

#### C5. HostStreamRegistry — handler creates, WASM polls

The existing `StreamRegistry` (F28) is WASM-created, host-drained. We need the
reverse: handler creates a queue, returns a stream ID to WASM, WASM polls it.
This is how `CameraSnapIPC` returns a stream of frames.

```rust
// foundation_platform/src/handle.rs

/// Handler-owned streams delivered to WASM.
/// Handler calls create() → gets stream_id. Returns stream_id in the IPC
/// response. WASM polls via host_ipc_stream_read(stream_id) import.
pub struct HostStreamRegistry {
    next_id: Mutex<u64>,
    streams: Mutex<BTreeMap<u64, HostStreamQueue>>,
}

pub type HostStreamQueue = Arc<ConcurrentQueue<Result<Vec<u8>, IpcError>>>;

impl HostStreamRegistry {
    /// Create a stream, return its ID. Handler writes chunks to the queue.
    pub fn create(&self) -> (u64, HostStreamQueue) { ... }
    /// Read the next chunk. WASM calls this via host_ipc_stream_read import.
    pub fn read(&self, stream_id: u64) -> Option<Result<Vec<u8>, IpcError>> { ... }
    /// Close and remove.
    pub fn close(&self, stream_id: u64) -> bool { ... }
}
```

The WASM FFI side provides the polling import:

```rust
// foundation_wasm/src/ipc.rs, #[cfg(target_family = "wasm")]

#[link(wasm_import_module = "platform")]
extern "C" {
    /// Read the next chunk from a host-created stream.
    /// Returns allocation_id (0 = stream closed or error).
    pub fn host_ipc_stream_read(stream_id: u64) -> u64;
}
```

Concrete flow: camera snap with streaming frames:

```
WASM: Camera::snap()  → host_ipc_invoke("camera", "snap", ...)
Host: NativeCamera::invoke_android()
        → opens camera intent
        → creates HostStreamRegistry entry → stream_id = 7
        → spawns background thread capturing frames into queue
        ← IpcResponse { payload: { stream_id: 7 } }

WASM: receives stream_id = 7
      loop: host_ipc_stream_read(7) → allocation → frame bytes
      host_ipc_stream_close(7) when done
```

#### C6. Two-tier dispatch on PlatformSession

```rust
// foundation_platform/src/session.rs

pub struct PlatformSession {
    pure_ipc_registry: IpcRegistry,     // Box<dyn Ipc>
    #[cfg(target_os = "android")]
    native_ipc_registry: RwLock<HashMap<String, Box<dyn Ipc + AndroidIpc>>>,
    #[cfg(target_os = "android")]
    android_handle: AndroidHandle,
    // ... same for ios ...

    handle_registry: CapabilityHandleRegistry,
    host_streams: HostStreamRegistry,
}

impl PlatformSession {
    pub fn invoke_ipc(
        &self,
        ctx: &IpcInvokeContext,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        // 1. Try native platform handler
        #[cfg(target_os = "android")]
        if let Some(h) = self.native_ipc_registry.read().unwrap().get(&request.ipc) {
            return h.invoke_android(ctx, &self.android_handle, request);
        }
        // 2. Fall back to pure Rust
        self.pure_ipc_registry.invoke(request)
    }
}
```

### Part D — `foundation_platform/src/wasm/`: typed WASM API (gap 4)

`foundation_platform` is a library crate that compiles for `wasm32` too. A
`#[cfg(target_family = "wasm")]` module provides typed structs wrapping
`host_ipc_dispatch()`. WASM apps bring in `foundation_platform` and get
`Camera::open()`, `Biometric::authenticate()`, etc. instead of raw FFI calls.

```
foundation_platform/
├── src/
│   ├── lib.rs              ← #[cfg(target_family = "wasm")] pub mod wasm;
│   ├── wasm/
│   │   ├── mod.rs           ← re-exports all capability wrappers
│   │   ├── camera.rs        ← pub struct Camera; impl Camera { pub fn open(...) }
│   │   ├── filesystem.rs    ← pub struct FilePicker; impl FilePicker { ... }
│   │   ├── biometric.rs     ← pub struct BiometricAuth; impl BiometricAuth { ... }
│   │   └── chrome.rs        ← pub struct Chrome; impl Chrome { ... }
│   ├── handle.rs            ← AndroidHandle, IosHandle, DesktopHandle,
│   │                           AndroidIpc, IosIpc, CapabilityHandleRegistry,
│   │                           HostStreamRegistry, IpcInvokeContext
│   ├── capability.rs        ← security layer (check_ipc_security)
│   ├── session.rs           ← updated with new registries + dispatch
│   └── builder.rs           ← updated Tauri commands
```

Each WASM wrapper struct is a thin typed layer over `host_ipc_dispatch`:

```rust
// foundation_platform/src/wasm/camera.rs
// Only compiled on wasm32 targets.

use foundation_wasm::ipc::{host_ipc_dispatch, IpcRequest, IpcResponse, IpcError};

pub struct Camera;

/// Arguments for opening the camera.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CameraOpenArgs {
    /// "front" | "back" | "default"
    pub facing: Option<String>,
    /// "photo" | "video"
    pub mode: Option<String>,
}

/// Handle returned after opening the camera.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CameraHandle {
    pub handle: u64,
}

/// Arguments for capturing a photo.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CaptureArgs {
    pub handle: u64,
}

/// Result of a photo capture.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct PhotoResult {
    pub uri: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
}

impl Camera {
    /// Open the camera. Returns a handle for subsequent capture/close calls.
    /// On web: returns `IpcError::ExecutionFailed("camera: unsupported on web")`.
    pub fn open(args: CameraOpenArgs) -> Result<CameraHandle, IpcError> {
        let req = IpcRequest {
            ipc: "camera".into(),
            action: "open".into(),
            payload: args,
            content_type: IpcContentType::Json,
            target: None,
        };
        host_ipc_dispatch::<CameraOpenArgs, CameraHandle>("camera", &req)
    }

    /// Capture a photo. Returns metadata + URI — not the bytes.
    /// The URI is an OS content URI (Android) or asset path (iOS).
    /// Use `FilePicker::read_bytes(uri)` to get the actual image data.
    pub fn capture(args: CaptureArgs) -> Result<PhotoResult, IpcError> {
        let req = IpcRequest {
            ipc: "camera".into(),
            action: "capture".into(),
            payload: args,
            content_type: IpcContentType::Json,
            target: None,
        };
        host_ipc_dispatch::<CaptureArgs, PhotoResult>("camera", &req)
    }

    /// Close the camera and release the native resource.
    pub fn close(handle: u64) -> Result<(), IpcError> {
        let req = IpcRequest {
            ipc: "camera".into(),
            action: "close".into(),
            payload: CloseArgs { handle },
            content_type: IpcContentType::Json,
            target: None,
        };
        host_ipc_dispatch::<CloseArgs, ()>("camera", &req)
    }
}
```

On the **native side** (Android), the same `Camera` IPC handler implements
`AndroidIpc` and does the real JNI work. On **desktop**, it returns
`IpcError::ExecutionFailed("camera requires Android or iOS")`. On **web**,
`host_ipc_invoke` reaches the browser host which also returns unsupported —
unless the host implements a WebRTC fallback.

The WASM app just imports and calls:

```rust
// User's WASM app (compiled to wasm32-unknown-unknown)
use foundation_platform::wasm::camera::{Camera, CameraOpenArgs, CaptureArgs};

let cam = Camera::open(CameraOpenArgs { facing: Some("back".into()), mode: Some("photo".into()) })?;
let photo = Camera::capture(CaptureArgs { handle: cam.handle })?;
// photo.uri = "content://android.media.provider/image/42"
// Use FilePicker to read the bytes if needed
Camera::close(cam.handle)?;
```

## Requirements

### R1. Merge WasmCapability into Ipc — `foundation_wasm`
- Delete `WasmCapability`, `CapabilityRequest`, `CapabilityResponse`,
  `CapabilityError`, `CapabilityContentType`, `CapabilityRegistry`
- `Ipc<Input = Vec<u8>, Output = Vec<u8>` — generic I/O, wire defaults
- `IpcKind::Capability { min_profile }`
- `IpcError` absorbs all old `CapabilityError` variants
- Single `IpcRegistry` in `foundation_wasm`

### R2. FFI bindings — `foundation_wasm`
- `host_ipc_invoke(ptr, len) -> allocation_id` — WASM→host invoke
- `host_ipc_stream_open(ptr, len) -> stream_id` — WASM→host stream open
- `host_ipc_stream_read(stream_id) -> allocation_id` — WASM polls host stream
- `host_ipc_stream_close(stream_id)` — close host stream
- `ipc_handle_event(ptr, len) -> u64` — host→WASM event push
- `host_ipc_dispatch::<T, U>(name, &IpcRequest<T>) -> Result<IpcResponse<U>, IpcError>` — typed helper
- `#[link(wasm_import_module = "platform")]`

### R3. Platform handles + context — `foundation_platform/src/handle.rs`
- `AndroidHandle { jni_env, activity, webview }` — `#[cfg(target_os = "android")]`
- `IosHandle { view_controller, window, webview }` — `#[cfg(target_os = "ios")]`
- `DesktopHandle { window_label }` — `#[cfg(not(any(...)))]`
- `IpcInvokeContext { page, webview_label, session }` — calling context
- `AndroidIpc` trait — `invoke_android(&self, ctx, handle, request)`
- `IosIpc` trait — `invoke_ios(&self, ctx, handle, request)`
- `CapabilityHandleRegistry` — token→resource map
- `HostStreamRegistry` — handler-created streams for WASM polling

### R4. Two-tier dispatch — `foundation_platform/src/session.rs`
- `pure_ipc_registry` — `Box<dyn Ipc>`, works everywhere
- `native_ipc_registry` — `Box<dyn Ipc + AndroidIpc>`, platform handlers
- `invoke_ipc()` — native first, pure fallback
- `android_handle` / `ios_handle` on session
- `handle_registry: CapabilityHandleRegistry`
- `host_streams: HostStreamRegistry`

### R5. Typed WASM API — `foundation_platform/src/wasm/`
- `#[cfg(target_family = "wasm")]` module
- One file per capability domain: `camera.rs`, `filesystem.rs`, `biometric.rs`, `chrome.rs`
- Each provides a struct with typed methods wrapping `host_ipc_dispatch`
- WASM apps depend on `foundation_platform` and get typed APIs
- On web/desktop where no native handler exists: `host_ipc_invoke` reaches the
  host, host returns `ExecutionFailed("unsupported on this platform")`

### R6. Backward compatibility
- Remove `PlatformCapability`, old `CapabilityRegistry`, `__ewe_capabilities`
- All capability invocations go through `__ewe_ipc`
- Example app updated: `EchoCap` → `IpcKind::Capability`
- `foundation_wasm::WasmCapability` deleted — WASM uses `Ipc` + typed wrappers

## Part E — JS runtime: IPC bridge in foundation-wasm.js

### E1 — foundation-wasm.js Web ABI additions

`foundation-wasm.js` already has protocol bytes 3 (capability trigger) and 4
(IPC trigger) for host→WASM dispatch. The WASM→host direction needs new host
imports that the WASM module calls via the FFI in `ipc_ffi.rs`.

Add to the `web_abi` object (the object passed as `{ abi: ... }` to
`WebAssembly.instantiate`):

```javascript
// foundation-wasm.js — additions to FoundationWasm.prototype._buildAbi()

web_abi = {
  // ... existing imports (memory, host_apply, schedule_timeout, etc.) ...

  // F41: IPC host invocation
  host_ipc_invoke: (ptr, len) => {
    return this._dispatchIpcInvoke(ptr, len);
  },
  host_ipc_stream_open: (ptr, len) => {
    return this._dispatchIpcStreamOpen(ptr, len);
  },
  host_ipc_stream_read: (streamId) => {
    return this._dispatchIpcStreamRead(streamId);
  },
  host_ipc_stream_close: (streamId) => {
    return this._dispatchIpcStreamClose(streamId);
  },
};
```

`_dispatchIpcInvoke(ptr, len)`:
1. Read the serialized `IpcRequest<Vec<u8>>` bytes from WASM linear memory
2. Deserialize via `ipcDecodeRequest(bytes)` — the JS mirror of
   `ipc::encode_request()` / `ipc::decode_response()`
3. Call `this._ipcInvokeHandler(request)` — if registered, returns a response
4. Allocate response bytes in WASM arena via `this.memory.allocate(len)`
5. Write encoded response, return allocation ID
6. If no handler, return 0

`_dispatchIpcStreamOpen(ptr, len)`:
1. Decode the request from memory
2. Create a `HostStream` — an internal queue + stream ID
3. Call handler to start producing chunks
4. Return stream ID (0 = error)

`_dispatchIpcStreamRead(streamId)`:
1. Pop next chunk from the stream's queue
2. Allocate in WASM arena, write chunk, return allocation ID
3. If closed: return 0

The **`_ipcInvokeHandler`** is set by the host (Tauri, browser bridge, Deno) —
NOT by the WASM module. It's how the JS side connects to the platform's IPC
registry. For Tauri this is the `__ewe_ipc` Tauri command. For browser this
calls the WASM module's `invoke_ipc` export directly.

### E2 — Binary wire format: JS mirror

Same encode/decode as Rust `ipc.rs`. JS functions:

```javascript
function ipcEncodeRequest(req) {
  // ipc name + action + ct + target + payload → Uint8Array
  var buf = new Uint8Array(4 + enc(req.ipc).length + ...);
  // ... length-delimited encoding matching Rust's encode_request()
  return buf;
}

function ipcDecodeResponse(data) {
  // Parse Uint8Array → { content_type, payload }
  var ct = data[0];
  var plen = new DataView(data.buffer).getUint32(1, true);
  return { content_type: ct, payload: data.slice(5, 5 + plen) };
}
```

### E3 — Remove capability-bridge.js

The old `capability-bridge.js` is replaced. Protocol byte 3 (capability) and 4
(IPC) are now unified. A new `ipc-bridge.js` registers trigger handlers by
calling `FoundationWasm.registerTriggerHandlers()`. The host detection (Tauri,
Deno, browser) stays but routes through the unified IPC channel.

### E4 — Embedded JS constant

```rust
// foundation_wasm_ui/src/embedded.rs
pub const IPC_BRIDGE_JS: &str = include_str!("../runtimes/ipc-bridge.js");
```

And in `ScriptInjector::with_platform_runtimes()`, capability_bridge →
ipc_bridge.

## Part F — Tauri integration: __ewe_ipc wired to HostStreamRegistry

`__ewe_ipc` in `builder.rs` is updated to:
1. Capture `IpcInvokeContext` from the Tauri window
2. Call `session.invoke_ipc(&ctx, &request)` — two-tier dispatch
3. For streaming requests: create a `HostStreamRegistry` entry, spawn a
   background task, return the stream ID immediately
4. For events: the handler can call `session.emit_to_page(page, ...)`

## Part G — Integration tests

### G1 — `ipc` binary codec round-trip test (foundation_wasm)

`encode_request` → bytes → `decode_request` → assert identical. Same for
response. No wasm target needed — pure Rust, the binary codec is portable.

### G2 — FFI dispatch test (foundation_wasm, `#[cfg(not(target_family = "wasm"))]`)

Register a mock Ipc handler in a local `IpcRegistry`, test that
`ipc_dispatch()` serializes, calls through the stub path, and gets the
correct response back. The non-wasm stub path uses the registry directly
instead of the FFI import.

### G3 — Full IPC round-trip (foundation_platform)

Register an IPC handler with the platform registry, invoke through
`PlatformSession::invoke_ipc()`, verify the response. Already partially
covered by `capability_suite.rs` and `platform_integration.rs`.

### G4 — JS bridge wire format test (foundation_wasm, `#[cfg(feature = "web")]`)

A Deno test that:
1. Loads `foundation-wasm.js` runtime + `ipc-bridge.js`
2. Instantiates a test WASM module that calls `ipc_dispatch()`
3. Verifies the JS `host_ipc_invoke` handler receives the correct binary payload
4. Verifies the WASM module receives and decodes the response

## Verification

```bash
cargo test -p foundation_wasm -- ipc              # unified IPC types + binary codec
cargo test -p foundation_wasm -- ipc              # wire encode/decode round-trip
cargo test -p foundation_platform -- ipc           # two-tier registry + dispatch
cargo test -p foundation_platform -- handle        # handle registry + host streams
cargo test -p foundation_platform -- wasm          # typed WASM wrappers (wasm32 target)

# JS runtime
deno test --allow-read backends/foundation_wasm/runtime/tests/ipc-bridge.test.js
```

## Files

| File | Action |
|---|---|
| `backends/foundation_wasm/src/ipc.rs` | **DONE** — types, trait, registry, binary encode/decode |
| `backends/foundation_wasm/src/ipc_ffi.rs` | **DONE** — ipc_dispatch(), ipc_handle_event(), IPC_TRIGGER |
| `backends/foundation_wasm/src/host_runtime.rs` | **DONE** — `pub mod ipc` with host imports |
| `backends/foundation_wasm/src/capability.rs` | **DELETED** |
| `backends/foundation_platform/src/handle.rs` | **DONE** — handles, traits, registries |
| `backends/foundation_platform/src/capability.rs` | **DONE** — PlatformIpc, PlatformIpcRegistry |
| `backends/foundation_platform/src/wasm/*.rs` | **DONE** — typed WASM wrappers |
| `backends/foundation_wasm/runtime/foundation-wasm.js` | **UPDATE** — host_ipc_invoke, host_ipc_stream_* in web_abi |
| `backends/foundation_wasm_ui/runtimes/capability-bridge.js` | **DELETE** |
| `backends/foundation_wasm_ui/runtimes/ipc-bridge.js` | **NEW** — unified JS bridge, detects host, registers triggers |
| `backends/foundation_wasm_ui/src/embedded.rs` | Add IPC_BRIDGE_JS constant |
| `backends/foundation_platform/src/injector.rs` | Replace capability_bridge → ipc_bridge |
| `backends/foundation_platform/src/builder.rs` | Wire host_ipc_invoke to session.invoke_ipc() |
| `backends/foundation_wasm/tests/ipc_wire_tests.rs` | **NEW** — binary encode/decode round-trip tests |
| `backends/foundation_wasm/tests/ipc_dispatch_tests.rs` | **NEW** — FFI dispatch tests |
| `backends/foundation_wasm/runtime/tests/ipc-bridge.test.js` | **NEW** — Deno JS bridge test |
