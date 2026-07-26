# Connecting to WASM — foundation_wasm_ui

## How WASM apps use the UI framework

The UI framework runs entirely within the WASM module — no native dependency. It connects to the host through the JS bridge:

```
WASM (compiled from Rust)
  │
  ├─ html! macro → DOM tree (runs in WASM, creates DOM via JS bridge)
  ├─ Signal → reactive state (WASM-managed, DOM subscriptions via JS)
  ├─ EventDispatcher → DOM events → WASM callbacks
  └─ IPC calls → host_ipc_invoke → native handlers
```

## Boot sequence

1. `#[wasm_app] pub fn init()` → WASM entry point
2. WASM memory initialized, allocator wired
3. JS bridge instance stored (`this.bridge = wasmInstance`)
4. `Runtime` object created (memory + bridge refs)
5. App `init()` runs — DOM setup, signal wiring, IPC registration
6. WebView's `bundle.js` injects the `invokeIpc` global for button callbacks

## Calling native IPC from WASM UI

```rust
use foundation_platform_native::shared::modal_types::PresentArgs;
use foundation_platform_native::wasm::modal::Modal;

// In a button callback or signal effect:
html! {
    <button onclick={move |_| {
        let args = PresentArgs {
            route: "/app/settings".into(),
            style: Some("bottom_sheet".into()),
            title: Some("Settings".into()),
        };
        // This calls through the IPC bridge to native ModalIpc
        spawn_local(async move {
            match Modal::present(args).await {
                Ok(result) => log::info!("Modal: {}", result.modal_id),
                Err(e) => log::error!("Failed: {:?}", e),
            }
        });
    }}>
        "Open Settings"
    </button>
}
```

## Building and deploying

```bash
# Build the WASM app
cargo build --target wasm32-unknown-unknown -p platform_android

# The build.rs WasmBundleGenerator produces:
#   public/app/v0.1.0/platform_dashboard.wasm
#   public/app/v0.1.0/platform_dashboard.js
#   public/app/v0.1.0/bundle.js
#   public/app/v0.1.0/index.html

# These are bundled into the APK by Tauri's resource config:
# tauri.conf.json → bundle.resources → "public/app/v0.1.0/**/*"

# At runtime, the VFS overlay serves them from inside the APK:
# ewe://localhost/app/ → AssetResolverFs → APK zip entry
```
