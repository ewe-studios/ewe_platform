---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F24-script-injector"
this_file: "specifications/52-tauri-foundation-platform/features/F24-script-injector/feature.md"

status: pending
priority: critical
created: 2026-07-20

depends_on:
  - "F03-ewe-protocol"
  - "F19-wasm-annotation-target"
  - "F21-multi-app-distribution-and-webview"

tasks:
  completed: 0
  uncompleted: 9
  total: 9
  completion_percentage: 0%
---
# F24 — ScriptInjector: auto-inject foundation_wasm + ui + polyfills

## Problem

Every `ewe://` page currently includes the foundation_wasm runtime scripts via
`<script src="foundation-wasm.js">` and `<script src="bundle.js">` tags in the
generated HTML. This creates several issues:

1. **Duplication.** Every app bundle (app/, app-hello/, dashboard/) copies the
   same `foundation-wasm.js`, `foundation-wasm-ui.js`, and
   `platform-scheme-interceptor.js` files into its output directory. For N apps,
   that's N copies of identical runtime code.

2. **Version skew.** If the platform upgrades its WASM runtime, every app bundle
   must be regenerated. A stale app bundle ships an old runtime, leading to subtle
   bugs where the platform and WASM runtime disagree on the wire protocol.

3. **No OTA path for runtime updates.** The runtime JS files are either copied
   per-app or embedded in the `.so`. Neither can be updated without an APK rebuild.
   If a critical bug is found in the scheme interceptor or WASM bridge, there's no
   way to patch it over the wire.

4. **Bundle complexity.** `WasmBundleGenerator` with `single_runtime: false`
   copies 3-4 JS files per app. The generator must track which runtime files to
   copy, their versions, and their load order. This is infrastructure burden
   that belongs in the platform, not the code generator.

5. **Tauri already supports this.** Tauri v2's webview creation injects init
   scripts (`core.js`, `ipc.js`, `ipc-protocol.js`) into every webview. Our
   `platform-scheme-interceptor.js` is currently injected via `window.eval()`
   after page load — a workaround, not the intended mechanism.

## Solution

A **`ScriptInjector`** on `PlatformBuilder` / `PlatformSession` that registers
scripts for automatic injection into every webview during creation. Each script
resolves through a **disk → embedded fallback chain**:

1. **Primary**: read from `public/runtimes/{script}.js` on disk (OTA-updatable —
   push a new runtime file and it takes effect on next webview creation, no APK
   rebuild).
2. **Fallback**: use the static `&'static str` embedded in `foundation_wasm` and
   `foundation_wasm_ui` (always available, ships with the binary).

The `WasmBundleGenerator` copies runtime files to `public/runtimes/` during
bundling — once, to a shared location — instead of per-app. App bundles contain
only their own code.

### Static runtime sources — `foundation_wasm` + `foundation_wasm_ui`

Both crates expose their runtime JS as public constants so the platform can
embed them as the fallback:

```rust
// foundation_wasm/src/runtime_js.rs (gated behind `embedded-js` feature)
/// The foundation-wasm.js runtime, embedded at compile time.
pub const FOUNDATION_WASM_JS: &str = include_str!("../runtimes/foundation-wasm.js");

// foundation_wasm_ui/src/runtime_js.rs
/// The foundation-wasm-ui.js runtime, embedded at compile time.
pub const FOUNDATION_WASM_UI_JS: &str = include_str!("../runtimes/foundation-wasm-ui.js");

/// The platform-scheme-interceptor.js, embedded at compile time.
pub const SCHEME_INTERCEPTOR_JS: &str = include_str!("../runtimes/platform-scheme-interceptor.js");
```

These are always compiled in (the `embedded-js` feature on `foundation_wasm`
already exists for this purpose). The platform depends on these constants as
the fallback source.

### ScriptInjector — `foundation_platform::injector`

```rust
/// A collection of scripts injected into every webview on creation.
///
/// Scripts are injected in registration order. Each script resolves its
/// source through a disk → embedded fallback chain.
pub struct ScriptInjector {
    resource_root: PathBuf,
    scripts: Vec<InjectedScript>,
}

/// Describes where a script's source comes from.
pub enum ScriptSource {
    /// Read from disk at runtime. OTA-updatable — replace the file and new
    /// webviews pick it up without an APK rebuild.
    Disk {
        /// Path relative to `resource_root`, e.g. `public/runtimes/foundation-wasm.js`.
        relative_path: PathBuf,
    },
    /// Static source compiled into the binary. Always available as a fallback.
    Static {
        /// The JavaScript source text.
        source: &'static str,
    },
}

pub struct InjectedScript {
    /// Unique identifier, e.g. "foundation_wasm_runtime", "scheme_interceptor".
    pub id: String,
    /// The resolution chain. First source that resolves wins.
    pub sources: Vec<ScriptSource>,
    /// When to inject relative to other scripts.
    pub stage: InjectStage,
}

pub enum InjectStage {
    /// Inject before any page content loads (equivalent to <head> script).
    Early,
    /// Inject after DOM is available but before page scripts execute.
    DomReady,
    /// Inject after the page has fully loaded.
    Load,
}
```

#### Script resolution

```rust
impl ScriptInjector {
    /// Resolve a script to its final source text.
    ///
    /// For each source in `sources` (in order):
    ///   - `Disk`: check `{resource_root}/{relative_path}`. If it exists and is
    ///     readable, return its content.
    ///   - `Static`: return the embedded `&'static str`.
    ///
    /// Returns `None` if no source in the chain resolves.
    pub fn resolve(&self, script_id: &str) -> Option<String> { ... }
}
```

This means:
- On first launch (no `public/runtimes/` yet): Static fallback → embedded copies.
  Everything works out of the box.
- After `WasmBundleGenerator` runs: Disk source resolves → uses the on-disk copy.
  Same as before, but now from a shared location.
- After an OTA runtime update: Disk source is replaced → new webviews get the
  updated runtime. The APK is untouched.

### PlatformBuilder API

```rust
impl PlatformBuilder {
    /// Register a script for automatic injection into every webview.
    pub fn inject_script(mut self, script: InjectedScript) -> Self { ... }

    /// Convenience: inject all standard platform runtime scripts with the
    /// disk → embedded fallback chain.
    ///
    /// Registers (in order):
    /// 1. `scheme_interceptor` — Disk: `public/runtimes/platform-scheme-interceptor.js`
    ///                              Fallback: `foundation_wasm_ui::SCHEME_INTERCEPTOR_JS`
    /// 2. `foundation_wasm`      — Disk: `public/runtimes/foundation-wasm.js`
    ///                              Fallback: `foundation_wasm::FOUNDATION_WASM_JS`
    /// 3. `foundation_wasm_ui`   — Disk: `public/runtimes/foundation-wasm-ui.js`
    ///                              Fallback: `foundation_wasm_ui::FOUNDATION_WASM_UI_JS`
    pub fn inject_platform_runtimes(self) -> Self { ... }

    /// Set the resource root for disk-based script resolution.
    pub fn resource_root(mut self, root: PathBuf) -> Self { ... }
}
```

### How injection works

Tauri v2 creates webviews in `manager/webview.rs`. During webview creation,
init scripts are loaded and evaluated before any page content. The platform
hooks into this via Tauri's `WebviewBuilder::initialization_script()`:

```
PlatformBuilder::build()
  → ScriptInjector::resolve() for each registered script
    → Disk: check {resource_root}/public/runtimes/{script}.js
      → exists? use it.
      → doesn't exist? fall through.
    → Static: use embedded &'static str
  → for each webview created:
    → WebviewBuilder::initialization_script(resolved_source)
      → Tauri/WRY evaluates the script before page load
```

For Android, WRY's `with_ipc_handler` and `with_initialization_script` serve
the same purpose — the scripts are evaluated in the WebView's context before
the `ewe://` page loads.

### WasmBundleGenerator: copy runtimes to `public/runtimes/`

The `WasmBundleGenerator` gains a new behavior: when building for the
platform target, it copies the runtime JS files to `public/runtimes/` —
a single shared location — instead of per-app directories:

```
public/
├── runtimes/                           # ← NEW: shared runtime files
│   ├── foundation-wasm.js              #     copied once by WasmBundleGenerator
│   ├── foundation-wasm-ui.js           #     OTA can replace these files
│   └── platform-scheme-interceptor.js  #
├── app/                                # app-specific bundle only
│   ├── index.html
│   └── bundle.js                       # no runtime scripts in here
└── app-hello/
    ├── index.html
    └── bundle.js
```

```rust
// WasmBundleGenerator config (in app build.rs)

#[wasm_bundle(
    single_runtime = true,          // Combine app JS + WASM into single bundle.js
    skip_runtime_files = true,      // Don't copy runtime files into THIS app's dir
    copy_runtimes_to_shared = true, // NEW: copy runtimes to public/runtimes/ instead
)]
struct MyApp;
```

When `copy_runtimes_to_shared` is set:
- The generator copies `foundation-wasm.js`, `foundation-wasm-ui.js`, and
  `platform-scheme-interceptor.js` to `public/runtimes/`
- If the directory doesn't exist, it creates it
- If files already exist, they are overwritten (idempotent)
- The generator reads the source files from `foundation_wasm/runtimes/` and
  `foundation_wasm_ui/runtimes/` via their crate-level `&'static str` exports,
  or directly from the source tree via `CARGO_MANIFEST_DIR`-relative paths

Generated HTML simplifies from:
```html
<script src="foundation-wasm.js"></script>
<script src="foundation-wasm-ui.js"></script>
<script src="platform-scheme-interceptor.js"></script>
<script src="bundle.js"></script>
```

To:
```html
<!-- Runtime scripts are injected by the platform -->
<script src="bundle.js"></script>
```

### What gets auto-injected

Three standard scripts, registered by `inject_platform_runtimes()`:

| ID | Disk path | Static fallback | Stage | Purpose |
|----|-----------|-----------------|-------|---------|
| `scheme_interceptor` | `public/runtimes/platform-scheme-interceptor.js` | `foundation_wasm_ui::SCHEME_INTERCEPTOR_JS` | Early | `location.assign` polyfill, `ewe://` link handling |
| `foundation_wasm` | `public/runtimes/foundation-wasm.js` | `foundation_wasm::FOUNDATION_WASM_JS` | Early | WASM runtime: arena, `ProtocolDispatcher`, memory bridge |
| `foundation_wasm_ui` | `public/runtimes/foundation-wasm-ui.js` | `foundation_wasm_ui::FOUNDATION_WASM_UI_JS` | Early | Reactive UI runtime: `html!`, signals, DOM patching |

Additional scripts can be registered by:
- Platform features (e.g. `inject_script` for a capability's JS bridge)
- User apps via `PlatformBuilder::inject_script()`
- Plugins that implement `ScriptInjectorPlugin`

### Plugin architecture

```rust
/// Plugins can register scripts with the injector during setup.
pub trait ScriptInjectorPlugin: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn inject_scripts(&self, injector: &mut ScriptInjector);
}

impl PlatformBuilder {
    pub fn plugin(mut self, plugin: impl ScriptInjectorPlugin) -> Self {
        plugin.inject_scripts(&mut self.script_injector);
        self
    }
}
```

The capability bridge (F23), IPC bridge (F25), and streaming channel bridge (F26)
each register their JS runtime via a plugin. Plugins that ship their own JS files
also copy them to `public/runtimes/` so they participate in the same OTA-updatable
disk → embedded fallback chain.

### OTA update flow

```
1. Backend pushes new runtime to device:
   PUT /ota/runtimes/foundation-wasm.js
   → saved to public/runtimes/foundation-wasm.js

2. Next webview creation:
   ScriptInjector::resolve("foundation_wasm")
   → Disk: public/runtimes/foundation-wasm.js exists → use it
   → Static fallback: skipped

3. If OTA file is corrupted or deleted:
   ScriptInjector::resolve("foundation_wasm")
   → Disk: public/runtimes/foundation-wasm.js missing → skip
   → Static: foundation_wasm::FOUNDATION_WASM_JS → use it
   // App still works with the shipped runtime.
```

## Requirements

### 1. Static runtime exports — `foundation_wasm` + `foundation_wasm_ui`
- `foundation_wasm/src/runtime_js.rs` (NEW) — `FOUNDATION_WASM_JS: &str` via `include_str!`
- `foundation_wasm_ui/src/runtime_js.rs` (NEW) — `FOUNDATION_WASM_UI_JS` + `SCHEME_INTERCEPTOR_JS`
- Gated behind existing `embedded-js` feature on `foundation_wasm`
- Always compiled on `foundation_wasm_ui` (it's always embedded)
- Re-exported from crate root so `foundation_platform` can depend on them

### 2. `ScriptInjector` struct — `foundation_platform`
- File: `backends/foundation_platform/src/injector.rs` (NEW)
- `ScriptSource` enum: `Disk { relative_path }` | `Static { source: &'static str }`
- `InjectedScript` with `id`, `sources: Vec<ScriptSource>`, `stage`
- `ScriptInjector` collection with registration-order preservation
- `resolve(script_id) -> Option<String>` — disk-first fallback chain
- Thread-safe: resolution happens at webview creation time

### 3. PlatformBuilder integration
- `inject_script(script)` method on builder
- `inject_platform_runtimes()` — registers all 3 standard scripts with disk → static chains
- `resource_root(root)` — sets the base path for `Disk` source resolution
- `ScriptInjector::resolve()` called during `build()`, results passed to
  Tauri's `WebviewBuilder::initialization_script()`
- Works on Desktop (Linux/macOS/Windows) and Mobile (Android/iOS)

### 4. WasmBundleGenerator: copy runtimes to shared location
- New config option: `copy_runtimes_to_shared: bool` (default: `false`)
- When `true`: copies `foundation-wasm.js`, `foundation-wasm-ui.js`,
  `platform-scheme-interceptor.js` to `public/runtimes/`
- Source: reads from `foundation_wasm/runtimes/` and `foundation_wasm_ui/runtimes/`
  via crate exports or `CARGO_MANIFEST_DIR`-relative paths
- Creates `public/runtimes/` directory if it doesn't exist
- Idempotent: overwrites existing files (safe to run repeatedly)
- Existing `skip_runtime_files: bool` (default: `false`) — when `true`, does NOT
  copy runtime files into the per-app output directory
- HTML template omits runtime `<script>` tags when `skip_runtime_files: true`
- `single_runtime: true` behavior unchanged (still combines app JS + WASM)
- Default: `copy_runtimes_to_shared=false`, `skip_runtime_files=false`
  (backward compatible — standalone WASM apps unaffected)

### 5. HTML template cleanup
- `build_tools/mod.rs`: conditionally omit runtime `<script>` tags
- Template parameter: `include_runtime_scripts: bool`
- Default `true` for backward compatibility (standalone WASM apps)

### 6. Android example migration
- `setup_routes()` calls `builder.resource_root(...).inject_platform_runtimes()`
- Generated `app/` and `app-hello/` bundles use `skip_runtime_files: true`
- First build: WasmBundleGenerator copies runtimes to `public/runtimes/`
- Verify via Chrome DevTools: runtime functions available before page scripts

### 7. OTA update path
- `public/runtimes/` is in the resource directory, readable by the platform
- Replacing a file in `public/runtimes/` takes effect on next webview creation
- If an OTA file is deleted or corrupted, the static fallback kicks in
- No APK rebuild needed for runtime updates

### 8. Plugin trait
- `ScriptInjectorPlugin` trait for external script registration
- `PlatformBuilder::plugin()` method
- Plugins can register their own scripts with the same disk → static fallback
- F23 capability bridge, F25 IPC bridge, F26 streaming channels register via plugins

### 9. No breaking changes
- Standalone WASM apps (non-platform) continue to include runtime scripts in HTML
- `foundation_wasm_ui` and `WasmBundleGenerator` unchanged when
  `skip_runtime_files` is `false` (default)
- All existing tests pass without modification

## Verification

```bash
# Static exports compile and contain real JS
cargo test -p foundation_wasm -- runtime_js
cargo test -p foundation_wasm_ui -- runtime_js

# ScriptInjector resolution (disk + static fallback)
cargo test -p foundation_platform -- script_injector

# WasmBundleGenerator: copy_runtimes_to_shared
cargo test -p foundation_wasm_ui -- wasm_bundle_copy_runtimes

# OTA fallback: delete disk file, verify static fallback used
cargo test -p foundation_platform -- script_injector_fallback

# Android: runtime scripts available before page load
# Chrome DevTools → Console:
# > typeof FoundationWasm         // "object" (not "undefined")
# > typeof ProtocolDispatcher     // "object"
# > typeof __ewe_interceptor      // "object"

# Android: OTA simulation — replace public/runtimes/foundation-wasm.js
# with a version that sets window.__OTA_TEST = true, restart webview
# > window.__OTA_TEST             // true
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_wasm/src/runtime_js.rs` | **NEW** — `FOUNDATION_WASM_JS` static export |
| `backends/foundation_wasm/src/lib.rs` | Add `pub mod runtime_js;` (gated on `embedded-js`) |
| `backends/foundation_wasm_ui/src/runtime_js.rs` | **NEW** — `FOUNDATION_WASM_UI_JS` + `SCHEME_INTERCEPTOR_JS` |
| `backends/foundation_wasm_ui/src/lib.rs` | Add `pub mod runtime_js;` |
| `backends/foundation_platform/src/injector.rs` | **NEW** — `ScriptInjector`, `ScriptSource`, `InjectedScript`, `InjectStage`, `ScriptInjectorPlugin` |
| `backends/foundation_platform/src/builder.rs` | Add `inject_script()`, `inject_platform_runtimes()`, `resource_root()`, `plugin()` |
| `backends/foundation_platform/src/lib.rs` | Add `pub mod injector;` |
| `backends/foundation_wasm_ui/src/build_tools/mod.rs` | Add `copy_runtimes_to_shared`, conditional HTML template |
| `backends/foundation_wasm_ui/src/build_tools/config.rs` | Add `copy_runtimes_to_shared: bool` + `skip_runtime_files: bool` to config |
| `examples/platform_android/src-tauri/src/lib.rs` | Call `resource_root(...).inject_platform_runtimes()`, regenerate bundles |
