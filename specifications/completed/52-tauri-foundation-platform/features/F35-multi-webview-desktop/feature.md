---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F35-multi-webview-desktop"
this_file: "specifications/52-tauri-foundation-platform/features/F35-multi-webview-desktop/feature.md"

status: merged
priority: high
created: 2026-07-21
updated: 2026-07-22

depends_on:
  - "F06-webview-stack"

tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 100%
---

# F35 — Multi-WebView desktop (MERGED INTO F06)

**Resolution (2026-07-22):** Merged into F06.

F35 was scoped as "desktop-only multi-WebView" based on the incorrect
assumption that Tauri mobile doesn't support multiple WebViews. Tauri v2
DOES support multiple WebViews on Android — each `WebviewWindowBuilder::new()`
creates a new activity/fragment with its own WebView.

The Hotwire Native model (new WebView per push, alive previous WebViews,
native back gesture) works identically on desktop and Android. The code
in F06 was updated to reflect this:

- F06 now defines the universal multi-WebView model
- `WindowManager` + `WindowOps` + `TauriWindowOps` already exist
- `WebViewPool` + `WebViewStack` + `PooledWebView` already exist
- `record_presentation` needs to be wired to WindowManager (remaining task)

The single-window screenshot-swap fallback for desktop is optional
and deferred — it's NOT the primary model.

**Status: Merged into F06. No separate work required.**
