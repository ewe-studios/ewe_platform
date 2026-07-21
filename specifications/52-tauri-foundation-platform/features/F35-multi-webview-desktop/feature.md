---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F35-multi-webview-desktop"
this_file: "specifications/52-tauri-foundation-platform/features/F35-multi-webview-desktop/feature.md"

status: completed
priority: high
created: 2026-07-21

depends_on:
  - "F29-platform-completeness"
  - "F06-webview-stack"

tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---

# F35 — Multi-WebView desktop: Tauri window lifecycle + screenshot-swap

## Problem

F29 built the data structures (`WebViewPool`, `PooledWebView`, `WebViewState`,
`push_with_presentation`, `drain_preloads`) but they are pure data — no
actual Tauri WebView windows are created, hidden, shown, or destroyed.
The pool tracks labels like "app", "app-hello", "app-settings" but Tauri
never creates windows for them.

Decision 10 defines a screenshot-swap navigation model: capture the current
WebView as an image, show the screenshot instantly on back-navigation, then
restore the WebView in the background. None of this is wired.

## Solution

Wire `WebViewStack` into Tauri's `WebviewWindowBuilder` API. Three phases:

### Phase 1: Window creation from pool

When `PooledWebView::get_or_create("app")` is called, the platform shell
creates a real Tauri WebView window if one doesn't exist:

```rust
// In the platform shell's on_page_load or navigation handler:

fn ensure_webview(
    app_handle: &tauri::AppHandle,
    pool: &mut WebViewPool,
    label: &str,
    url: &str,
) {
    if pool.get(label).is_none() {
        let _window = tauri::WebviewWindowBuilder::new(
            app_handle,
            label,
            tauri::WebviewUrl::App(url.into()),
        )
        .title(format!("ewe — {label}"))
        .visible(false)  // created hidden, shown on first navigate
        .build()
        .expect("failed to create WebView window");

        pool.get_or_create(label);
        pool.set_state(label, WebViewState::Ready);
    }
}
```

### Phase 2: Screenshot-swap navigation

On `push()`:
1. Capture screenshot of current WebView (`webview.eval()` → `html2canvas`
   or native Tauri `webview_window.capture_screenshot()` — Tauri 2.4+)
2. Mark current slot's state → Screenshot, store the PNG bytes
3. Create (or reuse) the target WebView window
4. Navigate the new WebView to the route
5. Hide old window, show new window

On `pop()`:
1. Show the previous slot's screenshot instantly (PNG → `<img>` overlay)
2. Restore the previous WebView in the background
3. Once the WebView finishes loading, swap the screenshot for the live view

On `replace()`:
1. Same window, different content — just navigate the existing WebView

### Phase 3: Background preload

When the user is on a page, the platform scans linked routes and preloads
them into idle WebViews. If a preloaded page matches the user's next
navigation, the WebView is already loaded — instant transition.

```rust
fn preload_linked_pages(
    pool: &mut WebViewPool,
    current_route: &str,
    links: &[String],
) {
    for link in links {
        if let Some(idle) = pool.find_idle() {
            let label = idle.label.clone();
            pool.set_state(&label, WebViewState::Loading);
            // Navigate the hidden WebView to the linked page
            // Mark ready when load completes
            pool.set_route(&label, link);
        } else {
            break; // no idle WebViews left
        }
    }
}
```

## Requirements

### R1. Tauri window creation from pool
- `ensure_webview(app_handle, pool, label, url)` creates `WebviewWindowBuilder`
- Windows created hidden by default (`.visible(false)`)
- Pool entry created/updated with state = Ready
- Called from `execute_decision()` when `Presentation::Push | Modal` and
  target label requires a new window

### R2. Screenshot capture
- `capture_screenshot(webview) -> Vec<u8>` — uses Tauri 2.4's
  `webview_window.capture_screenshot()` or fallback to `webview.eval(html2canvas)`
- PNG bytes stored in the `WebViewSlot.screenshot` field
- Screenshots evicted on memory budget (already implemented in F29)

### R3. Screenshot-swap on push
- `push_with_presentation()` calls screenshot capture before navigating
- Old window hidden: `window.hide().ok()`
- New window shown: `window.show().ok()`, `window.set_focus().ok()`

### R4. Screenshot-swap on pop
- Show previous screenshot as `<img>` overlay via `webview.eval()`
- Navigate the WebView to the previous route in background
- On `PageLoadEvent::Finished`, remove the overlay → live page visible
- If content is stale (marked by cache invalidation), reload after showing

### R5. Background preload
- After each `push()`, scan the new page's links (via `document.querySelectorAll('a[href]')`)
- Preload up to `max_preload` (default 2) linked pages into idle WebViews
- Preload cancels if user navigates before load completes
- Controlled by `RouteDecision.cache_policy` — only preload CacheFirst routes

### R6. Window lifecycle
- Windows created lazily (first navigation to their label)
- Windows NOT destroyed on pop — kept in pool for fast re-navigation
- Pool size capped by `StackConfig.max_windows` (default 4)
- Eviction: LRU idle window closed via `window.close()` when pool is full

### R7. Presentation mode wiring
- `Presentation::Morph` → same window, `webview.navigate(url)` (current behavior)
- `Presentation::Replace` → same window, `webview.navigate(url)` with screenshot
- `Presentation::Push` → new window, screenshot old, navigate new
- `Presentation::Modal` → floating window overlay, same as Push but window is smaller
- `Presentation::Root` → close all other windows, set root slot
- `Presentation::External` → `shell.open(url)` (system browser)

### R8. ViewKind routing
- `ViewKind::WebView` → route to named WebView via pool
- `ViewKind::NativeView` → deferred (SwiftUI/Jetpack Compose post-MVP)
- `decision.target` sets the WebView label; defaults to "main"

### R9. Platform shell integration
- `PlatformBuilder::build()` registers a `on_page_load` hook that calls
  `ensure_webview()` on every navigation
- The session's `webview_stack` is the source of truth for window state
- Tauri window events (close, focus, blur) update the pool

### R10. Desktop-only (v2)
- Multi-WebView is desktop-only for now
- Mobile (Android/iOS) uses single-WebView + screenshot model (F06)
- The pool is disabled on mobile — `WebViewStack::pool` remains None

## Verification

```bash
# Unit tests: pool + screenshot-swap logic
cargo test -p foundation_platform -- stack

# Integration: multi-window navigation
cargo test -p foundation_platform --test platform_integration -- multi_webview

# Manual: desktop app with 3 windows
cargo run --example platform_android
# Navigate: /app/ → /app-hello/ → /app/settings/
# Back button → screenshot-swap → live page
# Preload: idle WebView loads /app/ before user navigates back
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_platform/src/stack.rs` | Wire `WebViewPool` into Tauri window API |
| `backends/foundation_platform/src/builder.rs` | Register `on_page_load` hook for pool creation |
| `backends/foundation_platform/src/session.rs` | `execute_decision()` calls `ensure_webview()` |
| `backends/foundation_platform/src/window.rs` | **NEW** — Tauri window management: create, hide, show, screenshot |
