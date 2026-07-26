# Adding Native Capabilities — foundation_platform_native

A native capability has these components:

1. **Shared types** (`src/shared/`) — Wire-format, `serde`, `WirePayload`
2. **Native handler** (`src/native/`) — `Ipc + PlatformIpc` impl, registered on session
3. **WASM wrapper** (`src/wasm/`) — Typed API for WASM apps
4. **Kotlin/Swift helper** (if needed) — Android Activity or native UI

## Step-by-step: adding `barcode_scanner`

### 1. Shared types (`src/shared/barcode_types.rs`)

```rust
use foundation_wasm::ipc::{IpcContentType, WireError, WirePayload};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ScanArgs {
    pub format: Option<String>,  // "qr", "code128", etc.
    pub camera: Option<String>,  // "front" or "back"
}

impl WirePayload for ScanArgs {
    fn into_wire_bytes(self) -> (Vec<u8>, IpcContentType) {
        (serde_json::to_vec(&self).unwrap_or_default(), IpcContentType::Json)
    }
    fn from_wire_bytes(data: &[u8], _ct: IpcContentType) -> Result<Self, WireError> {
        serde_json::from_slice(data).map_err(|_| WireError::DecodeFailed)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ScanResult {
    pub code: String,
    pub format: String,
}
impl WirePayload for ScanResult { /* ... same pattern ... */ }
```

Register in `src/shared/mod.rs`:

```rust
#[cfg(feature = "barcode")]
pub mod barcode_types;
```

### 2. Native handler (`src/native/barcode.rs`)

```rust
use foundation_platform::ipc::{IpcCallback, PlatformIpc};
use foundation_platform::PlatformSession;
use foundation_wasm::ipc::{Ipc, IpcError, IpcKind, IpcRequest, IpcResponse};

use crate::shared::barcode_types::{ScanArgs, ScanResult};

pub fn register(session: Arc<PlatformSession>) {
    let ipc = BarcodeIpc { session: Arc::clone(&session) };
    session.register_ipc(ipc);
}

struct BarcodeIpc { session: Arc<PlatformSession> }

impl Ipc<Vec<u8>, Vec<u8>> for BarcodeIpc {
    fn name(&self) -> &str { "barcode" }
    fn kind(&self) -> IpcKind { IpcKind::Capability }
    fn invoke(&self, _req: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Err(IpcError::ExecutionFailed) // stub — unused, we override PlatformIpc
    }
}

impl PlatformIpc for BarcodeIpc {
    fn invoke_with_session(
        &self,
        req: &IpcRequest<Vec<u8>>,
        callback: IpcCallback,
    ) -> Result<(), IpcError> {
        let typed: IpcRequest<ScanArgs> = req.clone().into_typed()
            .map_err(|_| IpcError::InvalidPayload)?;

        // Call Kotlin/Swift via PluginHandle, or use OS-level APIs directly.
        // For Tauri mobile plugins, the pattern is:
        let app_handle = self.session.handles::<AppHandle<Wry>>()
            .ok_or(IpcError::ExecutionFailed)?;
        let handles = app_handle.state::<EweNativeHandles>();
        let result: ScanResult = handles.barcode
            .run_mobile_plugin("scanBarcode", typed.payload)
            .map_err(|_| IpcError::ExecutionFailed)?;

        callback(Ok(IpcResponse {
            payload: serde_json::to_vec(&result).unwrap_or_default(),
            content_type: IpcContentType::Json,
        }));
        Ok(())
    }
}
```

Register in `src/native/mod.rs`:

```rust
#[cfg(feature = "barcode")]
pub mod barcode;
```

### 3. WASM wrapper (`src/wasm/barcode.rs`)

```rust
use crate::shared::barcode_types::{ScanArgs, ScanResult};
use foundation_wasm::ipc::{dispatch_ipc, IpcError};

pub struct Barcode;

impl Barcode {
    pub async fn scan(args: ScanArgs) -> Result<ScanResult, IpcError> {
        let request = IpcRequest::new("barcode", "scan", args);
        let response: IpcResponse<ScanResult> = dispatch_ipc(request).await?;
        Ok(response.payload)
    }
}
```

Remember: the WASM wrapper only imports from `shared/` and calls the IPC dispatcher. It never touches Kotlin, Tauri, or `PlatformSession`.

Register in `src/wasm/mod.rs`:

```rust
#[cfg(feature = "barcode")]
pub mod barcode;
```

### 4. Kotlin/Swift helper (platform-specific, if needed)

For Android, add a new function to `EwePlatformPlugin.kt`:

```kotlin
@Command
fun scanBarcode(invoke: Invoke) {
    val args = invoke.parseArgs<ScanArgs>()
    // Launch barcode scanner Activity / CameraX
    val result = scanBarcodeImpl(args)
    invoke.resolve(result)
}
```

### 5. Feature flags (`Cargo.toml`)

```toml
[features]
default = ["modal", "dialog"]
modal = []
dialog = []
barcode = []
```

## Checklist

- [ ] Shared types with `WirePayload` impl
- [ ] Native handler with `PlatformIpc::invoke_with_session`
- [ ] `register(session)` function
- [ ] Module entry in `native/mod.rs`
- [ ] WASM wrapper with typed API
- [ ] Module entry in `wasm/mod.rs`
- [ ] Feature flag in `Cargo.toml`
- [ ] Kotlin/Swift helper (if platform UI needed)
- [ ] ProGuard keep rules (Android)
- [ ] Integration test in `tests/`
