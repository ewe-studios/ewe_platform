# Research — Android BottomSheet + WebView via Tauri Plugin Bridge

**Date:** 2026-07-25

## 1. How Tauri plugins call from Kotlin → Rust

The Tauri plugin bridge is documented in `tauri-2.11.5/src/plugin/mobile.rs`:

```
Kotlin calls: PluginManager.runCommand(id, pluginName, commandName, jsonData)
  ↓
JNI calls:   handle_android_plugin_response(env, id, success, error)
  ↓
Rust side:   PluginHandle::run_mobile_plugin::<T>(command, payload)
```

The Kotlin plugin extends `app.tauri.plugin.Plugin`:

```kotlin
class MyPlugin(activity: Activity) : Plugin(activity) {
    @Command
    fun myCommand(invoke: Invoke) {
        val args = invoke.parseArgs(MyArgs::class.java)
        // ... do work ...
        invoke.resolve(response)
    }
}
```

The `@Command` annotation registers the method. The `@TauriPlugin` annotation
generates the PluginManager integration code.

### Key finding: our IPC handlers can call PluginManager directly

We don't need `@Command` annotations. Our Kotlin helpers are called by PluginManager
commands dispatched from Rust:

```kotlin
// In EwePlatformPlugin.kt:
@Command
fun presentModal(invoke: Invoke) {
    val args = invoke.parseArgs(PresentModalArgs::class.java)
    val handle = ModalHelper(activity).presentModal(args.url, args.title, args.style)
    // Store the handle so dismissModal can find it
    activeModals[handle.id] = handle
    invoke.resolve(mapOf("modal_id" to handle.id))
}

@Command  
fun dismissModal(invoke: Invoke) {
    val args = invoke.parseArgs(DismissModalArgs::class.java)
    activeModals.remove(args.modalId)?.dismiss()
    invoke.resolve()
}
```

## 2. How BottomSheetDialogFragment works with WebView

```kotlin
class ModalHelper(private val activity: Activity) {
    fun presentModal(url: String, title: String, style: String): ModalHandle {
        return when (style) {
            "bottom_sheet" -> presentBottomSheet(url)
            "dialog" -> presentDialog(url, title)
            "fullscreen" -> presentFullscreen(url)
            else -> presentDialog(url, title)
        }
    }

    private fun presentBottomSheet(url: String): ModalHandle {
        val webView = createWebView(url)
        val dialog = BottomSheetDialog(activity)
        dialog.setContentView(webView)
        dialog.setOnDismissListener { /* notify Rust via PluginManager */ }
        dialog.show()
        return ModalHandle(dialog.hashCode().toString()) { dialog.dismiss() }
    }

    private fun presentDialog(url: String, title: String): ModalHandle {
        val webView = createWebView(url)
        val dialog = AlertDialog.Builder(activity)
            .setTitle(title)
            .setView(webView)
            .setNegativeButton("Close") { d, _ -> d.dismiss() }
            .create()
        dialog.show()
        return ModalHandle(dialog.hashCode().toString()) { dialog.dismiss() }
    }

    private fun createWebView(url: String): WebView {
        val webView = WebView(activity)
        webView.settings.javaScriptEnabled = true
        webView.settings.domStorageEnabled = true
        webView.loadUrl(url)
        // Size: match parent width, 80% height for bottom sheet
        val displayMetrics = activity.resources.displayMetrics
        webView.layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            (displayMetrics.heightPixels * 0.8).toInt()
        )
        return webView
    }
}
```

## 3. How Rust calls Kotlin via the Tauri plugin bridge

```rust
// In ModalIpc::present():

impl ModalIpc {
    fn present(session: &PlatformSession, req: &IpcRequest) -> Result<IpcResponse, IpcError> {
        let args: PresentModalArgs = serde_json::from_slice(&req.payload)?;

        // Call the Tauri plugin's Kotlin handler
        let plugin_handle = session.plugin_handle("ewe-platform-native")?;
        let result: serde_json::Value = plugin_handle
            .run_mobile_plugin("presentModal", &args)
            .map_err(|e| IpcError::ExecutionFailed(format!("{e}")))?;

        Ok(IpcResponse {
            payload: serde_json::to_vec(&result).unwrap(),
            content_type: IpcContentType::Json,
        })
    }
}
```

The `PluginHandle` comes from Tauri's plugin system — when the plugin is
registered via `tauri_plugin::Builder`, the app gets a handle that can call
`run_mobile_plugin::<T>(command, payload)`. This dispatches through JNI to
the Kotlin `@Command` method.

## 4. How WASM calls Rust through the IPC FFI

```rust
// foundation_platform_native/src/modal.rs, #[cfg(target_family = "wasm")]
pub mod wasm {
    use foundation_wasm::ipc_ffi::ipc_dispatch;

    impl Modal {
        pub fn present(args: PresentArgs) -> Result<PresentResult, IpcError> {
            ipc_dispatch("chrome", IpcRequest {
                ipc: "chrome".into(),
                action: "present_modal".into(),
                payload: serde_json::to_vec(&args)?,
                content_type: IpcContentType::Json,
                target: None,
            })
        }
    }
}
```

`ipc_dispatch` serializes → `host_ipc_invoke(ptr, len)` → alloc → deserialize.
The host (Tauri/Deno/browser) routes it to the IPC registry → `ModalIpc.invoke_with_session()`.
Already proven E2E by F41 integration tests.

## 5. How the Kotlin plugin gets into the APK

`tauri_plugin::Builder::android_path("android/")` in `build.rs` emits
`cargo:android_library_path=/path/to/android/`. The Tauri build system
picks this up and adds the directory as a Gradle library module.

Our `android/build.gradle.kts` declares:

```kotlin
plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.ewe.platform.native"
    compileSdk = 36
    defaultConfig { minSdk = 24 }
}

dependencies {
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.webkit:webkit:1.14.0")
}
```

## 6. What "done" looks like

- [ ] Kotlin plugin registered via `tauri_plugin::Builder` — `presentModal` command works
- [ ] Rust `ModalIpc` calls `plugin_handle.run_mobile_plugin("presentModal", ...)`  
- [ ] WASM `Modal::present()` calls `ipc_dispatch("chrome", "present_modal", ...)`
- [ ] Running `platform_android` on emulator — pressing "Open Bottom Sheet" button shows
      a BottomSheetDialogFragment with a WebView loading the requested route
- [ ] Dismissing the modal (swipe down or RemoteProgrammatic) sends event back to calling page
- [ ] All 379 existing tests pass
- [ ] New tests: `ModalIpc::present` handler logic, Kotlin plugin unit test (mock WebView)  
