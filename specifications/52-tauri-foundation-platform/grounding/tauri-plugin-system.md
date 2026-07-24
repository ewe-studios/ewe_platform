# Tauri Plugin System — Ground Truth (for F42 reference)

Studied from `tauri-2.11.5/src/plugin.rs`, `tauri-2.11.5/src/plugin/mobile.rs`,
and `tauri-plugin-2.6.3/src/build/`. Date: 2026-07-25.

## Three layers

### Layer 1 — `tauri::plugin::Plugin` trait (runtime)

```rust
pub trait Plugin<R: Runtime>: Send {
    fn name(&self) -> &'static str;
    fn initialize(&mut self, app: &AppHandle<R>, config: JsonValue) -> Result<...>;
    fn initialization_script(&self) -> Option<String> { None }
    fn window_created(&mut self, window: Window<R>) {}
    fn webview_created(&mut self, webview: Webview<R>) {}
    fn on_navigation(&mut self, webview: &Webview<R>, url: &Url) -> bool { true }
    fn on_page_load(&mut self, webview: &Webview<R>, payload: &PageLoadPayload) {}
    fn on_event(&mut self, app: &AppHandle<R>, event: &RunEvent) {}
    fn extend_api(&mut self, invoke: Invoke<R>) -> bool { false }
}
```

The `Builder` struct provides a convenience layer — users call
`Builder::new("name").setup(...).invoke_handler(...).build()`.

### Layer 2 — `tauri_plugin::Builder` (build time, in build.rs)

```rust
tauri_plugin::Builder::new(commands)
    .android_path("android/")     // copies this dir into the app as a library module
    .ios_path("ios/")             // copies this dir into the Xcode project
    .global_api_script_path("guest-js/index.js")  // injects JS into WebView
    .build();                      // generates ACL permissions, copies native files
```

This runs in `build.rs`. It:
1. Copies `android/` into the consuming app's Gradle project
2. Copies `ios/` into the consuming app's Xcode project
3. Generates `permissions/` TOML files from the command list
4. Writes `target/acl/` metadata consumed by the Tauri bundler

The `android/` directory is a Gradle library module. The plugin crate's
Kotlin code lives there. It gets compiled into the final APK.

### Layer 3 — Mobile bridge (JNI/FFI)

**Android:** The plugin's Kotlin class extends `app.tauri.plugin.Plugin`:

```kotlin
class EwePlatformPlugin(activity: Activity) : Plugin(activity) {
    override fun load(webView: WebView, config: String) {
        // config = JSON from Rust Plugin::initialize()
    }

    override fun onResume() { ... }
    override fun onStop() { ... }
}
```

To call from Kotlin → Rust: `PluginManager.runCommand(id, pluginName, commandName, jsonData)`.
The Rust side processes it via `PluginHandle::run_mobile_plugin::<T>(command, payload)`.

**iOS:** Same concept via FFI — `register_ios_plugin(init_fn)` + `run_plugin_command()`.

## How this maps to our needs

| Our need | Tauri provides |
|---|---|
| Copy Kotlin/Swift files into gen/ | `tauri_plugin::Builder::android_path()` / `ios_path()` |
| Gradle deps for native code | Declared in `android/build.gradle.kts` inside the plugin crate |
| Android permissions | Declared in `permissions/default.toml` + `AndroidManifest.xml` in the plugin dir |
| Kotlin↔Rust communication | `PluginManager.runCommand()` ↔ `PluginHandle::run_mobile_plugin()` |
| Register IPC handlers at startup | `tauri::plugin::Builder::setup()` — called during app init |
| Inject JS into WebView | `Builder::js_init_script()` or `Plugin::initialization_script()` |

## What we DON'T build

We do NOT need:
- A custom `NativeModule` trait — Tauri's `Plugin` trait IS the native module trait
- A `PlatformCodegen` pipeline — `tauri_plugin::Builder::build()` already copies files
- Manual `window.eval()` or script injection — `initialization_script()` does this properly
- Our own JNI bridge — `PluginManager.runCommand()` + `handle_android_plugin_response()` are built in

## What we DO build

1. A `foundation_platform_native` crate that IS a Tauri plugin
2. Per-module IPC handlers that register themselves in the plugin's `setup()` hook
3. Kotlin/Swift plugin classes that implement `app.tauri.plugin.Plugin`
4. WASM wrappers that call the IPC handlers through the unified FFI
