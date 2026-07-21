---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F37-surface-2-native-lib"
this_file: "specifications/52-tauri-foundation-platform/features/F37-surface-2-native-lib/feature.md"

status: pending
priority: medium
created: 2026-07-21

depends_on:
  - "F13-cross-platform-builds"
  - "F17-app-build-pipeline"

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# F37 — Surface 2: native static library (`.a`/`.so`)

## Problem

Surface 1 (WebView WASM) and Surface 3 (wasmtime WASM) both run WASM.
Surface 2 is the third option: compile Rust application logic as a NATIVE
static library (`.a` on iOS, `.so` on Android) linked directly into the
app binary. No WASM, no wasmtime, no WebView overhead — just a function call.

Decision 04 defines this as a deployment surface but no code exists for it.
The `#[platform_bin]` annotation exists but only produces WASM targets.
Users need `#[platform_native]` to compile Rust crates as `staticlib` or
`cdylib` targets linked into the Tauri binary.

## Solution

### `#[platform_native]` annotation

Marks a crate as a native library target. The build pipeline:
1. Scans for `#[platform_native]` (similar to `#[wasm_bin]`)
2. Compiles as `staticlib` (iOS) or `cdylib` (Android)
3. Links the `.a`/`.so` into the Tauri binary via `Cargo.toml` deps

```rust
// app-native/src/lib.rs

#[platform_native]
pub async fn handle_payment(
    amount: u64,
    currency: &str,
) -> Result<PaymentResult, PaymentError> {
    // This runs as native code — zero copy, zero serialization overhead.
    // Direct access to the session, filesystem, network, or hardware.
}
```

### Generated glue code

The codegen produces a type-safe bridge:

```rust
// Generated: src/generated/native/payment.rs
pub struct PaymentHandler;

impl PaymentHandler {
    pub fn handle_payment(amount: u64, currency: &str) -> Result<PaymentResult, PaymentError> {
        // Direct FFI call into the linked native library
        app_native::handle_payment(amount, currency)
    }
}
```

### Zero-copy data path

Surface 2's killer feature: same-process, zero-serialization. Data stays
in Rust memory. Arrow arrays pass by reference via `Arc<ArrayData>`.
No JSON encode/decode. No WASM linear memory copy. No FFI boundary
(static linking means same address space).

```rust
// The native function receives Arrow arrays directly:
fn process_batch(batch: &RecordBatch) -> RecordBatch {
    // batch columns are Rust slices — no copy, no IPC, no serialization
    let filtered: Vec<&dyn Array> = batch.columns().iter()
        .map(|col| filter_arrow(col))
        .collect();
    RecordBatch::try_new(batch.schema(), filtered).unwrap()
}
```

## Requirements

### R1. `#[platform_native]` annotation
- New `AnnotationKind::PlatformNative` in codegen.rs scanner
- `crate-type = ["staticlib", "cdylib"]` in the native crate's Cargo.toml
- Compiles for the host target (same as Tauri shell)

### R2. Build pipeline
- `build_native_app()` in codegen.rs — compiles the native crate
- Output copied to `src-tauri/native/` for linking
- `Cargo.toml` updated to add the `.a`/`.so` as a dependency

### R3. Generated glue
- `generate_native_modules()` — type-safe wrapper per annotated function
- `NativeResponder` implements `RouteResponder` — routes to a native function
- `native_handler("payment")` route constructor

### R4. Zero-copy Arrow path
- Native functions receive `&[u8]` refs to Arrow IPC bytes in shared memory
- No serialization — the caller passes pointers, the callee reads them
- Works because everything is in the same process

### R5. Platform targets
- iOS: `aarch64-apple-ios` → `.a` static library
- Android: `aarch64-linux-android` → `.so` shared library
- Desktop: same target as host → linked directly

### R6. App-store implications
- Native code changes require full app review (unlike WASM hot-swap)
- Documented as trade-off: performance vs update flexibility
- Surface 1 (WebView WASM) remains the default for app-store-avoidant updates

## Verification

```bash
# Build native app
cargo build -p app-native --target aarch64-apple-ios

# Verify linking
otool -L target/aarch64-apple-ios/debug/libapp_native.a

# Integration: native route handler
cargo test -p foundation_platform -- native_handler
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_platform/src/codegen.rs` | Add `build_native_app()` + `AnnotationKind::PlatformNative` |
| `backends/foundation_platform/src/responder.rs` | Add `NativeResponder` |
| `backends/foundation_platform/src/route.rs` | Add `native_handler()` constructor |
