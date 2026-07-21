---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F21-multi-app-distribution-and-webview"
this_file: "specifications/52-tauri-foundation-platform/features/F21-multi-app-distribution-and-webview/feature.md"

status: completed
priority: critical
created: 2026-07-18

depends_on:
  - "F16-app-crate-structure"
  - "F17-app-build-pipeline"

tasks:
  completed: 0
  uncompleted: 14
  total: 14
  completion_percentage: 0%
---
# F21 — Multi-App Distribution & `webview_app()` Asset Serving

## Problem

1. **One wasm crate = one flat `public/`.** When multiple `#[wasm_bin]`
   entrypoints exist, their `.wasm` + `.js` all land in `public/` with no
   namespacing. A second app crate (e.g. `app-hello/`, `app-admin/`)
   would collide.

2. **`webview_app()` doesn't know where its assets are.** The
   `BackendTransport::signal_webview()` method receives a route but has
   no way to serve the actual `.wasm` + `.js` files for that specific
   app. The `index.html` is hand-generated with hacks (inlining JS,
   stripping exports).

3. **WebView initial URL is hardcoded to `public/index.html`.** Tauri's
   `WebViewUrl::App("index.html")` loads through WebViewAssetLoader
   which strips scripts. The fix (`WebviewUrl::External`) needs to
   point to a per-app endpoint.

## Solution

### AppDistribution

```rust
/// Describes one WASM app crate that contributes assets to the build.
pub struct AppDistribution {
    /// Name used for the public/ subdirectory (e.g. "app-hello" → public/app-hello/)
    pub name: String,
    /// Path to the crate root (e.g. root.join("app-hello"))
    pub crate_dir: PathBuf,
    /// Route prefix this app serves at (e.g. "/app/hello/")
    pub route_prefix: String,
}
```

Root `build.rs`:
```rust
let apps = vec![
    AppDistribution {
        name: "app-hello".into(),
        crate_dir: root.join("app-hello"),
        route_prefix: "/app/hello/".into(),
    },
];

foundation_platform::codegen::build_all_wasm_apps(&apps, &public_dir);
```

Output:
```
public/
├── app-hello/
│   ├── platform_dashboard.wasm
│   ├── platform_dashboard.js
│   ├── foundation-wasm.js
│   ├── foundation-wasm-ui.js
│   ├── platform-scheme-interceptor.js
│   └── index.html           ← generated: imports platform_dashboard.js
├── app-admin/
│   └── ...
```

### `webview_app().with_assets(AppAssets)`

```rust
#[derive(EmbedDirectoryAs)]
#[source = "public/app-hello"]
struct AppHelloAssets;

session.route("/app/hello/*", webview_app()
    .with_profile(Profile::App)
    .with_assets(AppHelloAssets::default()));
```

`AppAssets` stores the directory path. `query_backend()` uses it to
know which public/ subdirectory to read files from when serving the
initial HTML or static assets for that route.

### Generated `src/generated/` wrappers

`generate_platform_code()` produces one file per `AppDistribution`:

```rust
// src/generated/app_hello.rs  (auto-generated)
use foundation_macros::EmbedDirectoryAs;

#[derive(EmbedDirectoryAs)]
#[source = "public/app-hello"]
struct AppHelloAssets;
```

And `src/generated/mod.rs`:
```rust
pub mod app_hello;
```

### WebView initial URL

`PlatformBuilder::build()` sets the main window's initial URL to
`http://ewe.localhost/app/hello/` when a route prefix is registered
with `webview_app()`. The wry workaround intercepts HTTP requests,
routes them to `ewe_handler`, which serves from the correct
`public/app-hello/` subdirectory. Scripts execute because the response
comes through `shouldInterceptRequest` with proper Content-Type.

## Requirements

### R1 — `AppDistribution` struct
- [ ] Defined in `foundation_platform::codegen`
- [ ] `name` → `public/{name}/` subdirectory
- [ ] `crate_dir` → path to wasm crate root
- [ ] `route_prefix` → URL prefix for this app

### R2 — `build_all_wasm_apps()` entry point
- [ ] Accepts `&[AppDistribution]` + `&Path` (public dir)
- [ ] For each app: scans `crate_dir/src/` for wasm annotations
- [ ] Compiles to wasm32, places artifacts in `public/{name}/`
- [ ] Generates `public/{name}/index.html` — minimal loader
- [ ] Copies JS runtimes to `public/{name}/`
- [ ] Runs WasmBundleGenerator per crate

### R3 — Generated `index.html` per app
- [ ] Minimal: `<script type=module src="./platform_dashboard.js"></script>`
- [ ] `<script type=module>import { init } from './platform_dashboard.js'; init()</script>`
- [ ] Status div for loading/error feedback
- [ ] NO JS inlining, NO export stripping, NO hacks

### R4 — `webview_app().with_assets()`
- [ ] `RouteDecisionExt` gains `.with_assets(impl EmbedDirectoryAs)`
- [ ] `RouteDecision` gains `asset_path: Option<String>` field
- [ ] `query_backend()` uses `asset_path` to resolve files

### R5 — WebView initial URL via ewe://
- [ ] `PlatformBuilder::build()` sets main window URL to
  `http://ewe.localhost/{route_prefix}` (first registered webview_app)
- [ ] `ewe_handler` serves `public/{name}/` files when path matches
- [ ] No `location.replace()` hack — proper initial URL

### R6 — `src/generated/` per app
- [ ] `generate_platform_code()` produces `src/generated/{name}.rs`
- [ ] Contains `#[derive(EmbedDirectoryAs)]` struct
- [ ] `mod.rs` auto-updated with `pub mod {name};`

### R7 — Emulator resolution fix
- [ ] Set display to 600x800 in AVD config permanently
- [ ] Apply via `hw.display1.width/height/density` in config.ini

### R8 — No codegen hacks
- [ ] Remove all JS inlining from `codegen.rs`
- [ ] Remove all `strip_exports` logic
- [ ] Remove all `format!` HTML generation in codegen
- [ ] `codegen.rs` is pure orchestrator: scan → delegate → copy

## Verification

```bash
# Build all apps
cargo build  # triggers root build.rs → build_all_wasm_apps

# Check output structure
ls public/app-hello/    # .wasm, .js, runtimes, index.html
ls public/app-admin/    # separate namespace

# Confirm generated modules
cat src-tauri/src/generated/app_hello.rs  # EmbedDirectoryAs struct
cat src-tauri/src/generated/mod.rs        # pub mod app_hello

# Desktop build
cargo build --package platform_demo

# Android build + deploy
cargo tauri android build
adb install ...
adb logcat | grep WASM   # "Step 1: FoundationWasm..." → "WASM active"
```
