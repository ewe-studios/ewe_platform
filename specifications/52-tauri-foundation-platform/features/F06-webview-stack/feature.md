---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F06-webview-stack"
this_file: "specifications/52-tauri-foundation-platform/features/F06-webview-stack/feature.md"

status: in-progress
priority: critical
created: 2026-07-17
updated: 2026-07-22

depends_on:
  - "F01-session-backbone"
  - "F02-route-handler"

tasks:
  completed: 7
  uncompleted: 4
  total: 11
  completion_percentage: 64%
---

# F06 — Multi-WebView stack manager (Hotwire Native model)

## Problem (revised 2026-07-22)

The original v1 spec described a single-WebView + screenshot-swap model.
This was wrong. Hotwire Native does NOT use screenshots — it creates a
**new native ViewController/Activity with its own WebView** on every push.
The old WebView stays alive. On pop, the previous screen is already
rendered — it's the native stack popping, not a screenshot hack.

Tauri v2 supports multiple WebViews on ALL platforms (desktop AND Android).
`WebviewWindowBuilder::new(app_handle, label, url)` creates a new WebView
backed by a new window/activity/fragment on every platform. Mobile does
not use screenshots — it uses the native back gesture.

## Solution: Hotwire Native model

Each `Presentation::Push` or `Presentation::Modal` creates a NEW WebView
window. The old WebView stays alive behind it. The pool tracks all
WebViews by label. On pop, the platform calls `window.close()` on the
top WebView, revealing the previous one — already rendered, instant.

```
Presentation::Push:
  1. WindowManager.ensure("wv_3", url) → creates new WebView window
  2. Pool: push new entry with label "wv_3", state Loading
  3. Previous slot state → Ready (hidden, alive)
  4. New slot state → Active (visible, focused)

Presentation::Pop (triggered by back):
  1. Pool: pop top slot
  2. WindowManager.close("wv_3") → destroys top WebView
  3. WindowManager.activate("wv_2") → shows previous WebView
  4. Previous WebView is already rendered — no screenshot, no reload

Presentation::Morph:
  1. Same WebView, in-place navigate
  2. Pool: update active slot's route, depth unchanged

Presentation::Replace:
  1. Same WebView, in-place navigate
  2. Pool: update active slot's route, depth unchanged

Presentation::Root:
  1. Destroy all WebViews except the new root
  2. Pool: clear, push single root slot
```

### No screenshots on mobile

Screenshots are a Basecamp v1 hack. Hotwire Native doesn't need them
because the previous WebView is already alive and rendered. On back,
you just show it — the WebView's DOM is still in memory with the full
page state (scroll position, form input, JS state). This is superior
to a frozen screenshot in every way.

On DESKTOP, if multi-window is enabled (user opted in), Push creates a
separate OS window. The screenshot-swap model is a fallback for
single-window desktop mode (F35's v2 scenario), but it's NOT the
primary model — it's an optional optimization for constrained UIs.

### Back navigation

The platform hooks Tauri's window close event. When a pushed WebView
is closed (back gesture on Android, Cmd+W on desktop), the platform:
1. Pops the stack
2. Activates the previous WebView
3. No screenshot, no reload — the WebView was alive the whole time

## Requirements

### R1. `WebViewSlot` — per-slot data ✅
- `route`, `page_identity`, `state: SlotState`, `is_content_stale`
- `webview_label: String` — the Tauri window label for this slot

### R2. `WebViewPool` — label → state tracking ✅
- `get_or_create(label)`, `get(label)`, `set_state()`, `set_route()`
- `find_idle()` for preload reuse

### R3. `WebViewStack` — navigation state machine ✅
- `push_with_presentation()` — branches per mode
- `pop()` — close top WebView, activate previous
- `morph()` / `replace()` — in-place navigate
- `set_root()` — destroy all, new root

### R4. `record_presentation` — wired to WindowManager 🔄
- Push/Modal → `WindowManager.ensure(label, url)` creates NEW WebView
- Morph/Replace → `WindowManager.navigate(label, url)` navigates existing
- Root → `WindowManager.close()` all except new root
- External → `tauri::api::shell::open()` system browser

### R5. `WindowManager` — per-window lifecycle ✅
- `ensure(label, url)` → create if missing, navigate if exists
- `activate(label)` → show + focus
- `deactivate(label)` → hide
- `destroy(label)` → close + remove from pool

### R6. Desktop single-window fallback ⚠️ deferred
- Single-window mode uses screenshot-swap as optimization
- Multi-window mode (default on desktop) uses the Push=NewWindow model
- Mobile always uses Push=NewWindow (native back gesture)

### R7. Back button integration — Tauri window close hook 🔄
- `window.on_close_requested()` → pool.pop() → activate previous
- Android back gesture → same flow

### R8. No screenshots for Push on mobile 🔄
- Previous WebView stays alive, hidden
- Back reveals it instantly — DOM state preserved
- Screenshots only used for single-window desktop fallback

## Files

| File | Action |
|------|--------|
| `backends/foundation_platform/src/stack.rs` | WebViewPool, WebViewStack, SlotState |
| `backends/foundation_platform/src/window.rs` | WindowManager, WindowOps, TauriWindowOps |
| `backends/foundation_platform/src/session.rs` | record_presentation → WindowManager wiring |
| `backends/foundation_platform/src/builder.rs` | PlatformBuilder injects TauriWindowOps |
| `backends/foundation_wasm_ui/runtimes/stack-viewport.js` | Optional: screenshot overlay for single-window |
