---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F06-webview-stack"
this_file: "specifications/52-tauri-foundation-platform/features/F06-webview-stack/feature.md"

status: pending
priority: high
created: 2026-07-17

depends_on:
  - "F01-session-backbone"
  - "F02-route-handler"

tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# F06 — Single-WebView stack manager (v1)

## Overview

Implement the WebView stack manager using the Basecamp single-WebView +
screenshot-swap model. v1 operates with one shared WebView per Tauri window
(on mobile and stable desktop). Screenshot capture, push/pop/morph navigation,
stale content detection, and back-gesture integration.

[Decision 10](../decisions/10-multi-webview-stack.md) defines the stack
manager architecture. v1 uses the single shared WebView model (Basecamp
pattern). Multi-WebView with `Window::add_child` is desktop+unstable only
(post-MVP).

## Dependencies

Depends on:
- `F01-session-backbone` — Stack registers as a session subsystem
- `F02-route-handler` — Navigation decisions drive push/pop/morph

Required by:
- `F08-walking-skeleton` — End-to-end navigation with stack transitions

## Requirements

### 1. `WebViewSlot` and `ScreenSlot`

```rust
// foundation_platform/src/stack.rs

struct WebViewSlot {
    route: String,
    page_identity: PageIdentity,
    webview: Option<Webview<R>>,
    screenshot: Option<Vec<u8>>,
    state: SlotState,
}

enum SlotState {
    Screenshot,      // Screenshot-only. No live WebView.
    Preloading,      // WebView is loading in the background.
    Ready,           // WebView is loaded and ready but not visible.
    Active,          // WebView is visible and active.
    Transitioning,   // Animation in progress.
}
```

For native view support (post-MVP), `ScreenSlot` wraps WebView or native:

```rust
enum ScreenSlot {
    WebView { ... },
    Native { view_handle: NativeViewHandle, state: SlotState },
}
```

### 2. WebView stack

```rust
struct WebViewStack<R: Runtime> {
    window: WebviewWindow<R>,
    slots: Vec<WebViewSlot>,
    active_index: usize,
    config: StackConfig,
}

struct StackConfig {
    max_screenshots: usize,             // default: 10
    screenshot_memory_budget: usize,    // default: 50MB
}
```

### 3. Navigation flow: push (Basecamp model)

```
1. User taps a link on the active screen.
2. Session backbone intercepts → route handler returns RouteDecision.
3. If presentation is Push:
   - Deactivate current screen: screenshot captured, slot state → Screenshot.
   - Move shared WebView to new screen: navigate to new URL.
   - Animate transition: old screenshot slides left, new content slides in.
4. Previous slot now shows its screenshot. It has no live WebView.
```

### 4. Navigation flow: pop

```
1. User taps back.
2. Stack manager pops the top slot.
3. Previous slot has a screenshot (Screenshot):
   - Show screenshot INSTANTLY.
   - Move shared WebView to this slot, navigate to route.
   - Screenshot → Active once content loads.
4. If content is stale, reload after visible.
```

### 5. Navigation flow: morph

No stack change. Current WebView stays active. Content is patched in-place
(by the rendering lane). No screenshot, no transition.

### 6. Screenshot capture

```rust
impl WebViewSlot {
    fn capture_screenshot(&mut self) {
        if let Some(webview) = &self.webview {
            // JS canvas-based capture (no native API available):
            // webview.evaluate_script("canvas.toDataURL()")
            // Store as base64 PNG
            self.screenshot = Some(...);
        }
    }

    fn show_screenshot(&self) {
        // CSS: position absolute, z-index above WebView
        // Fade in screenshot <img> element
        self.window.evaluate_script(&format!(
            "showScreenshot({}, '{}')",
            self.slot_index,
            base64::encode(self.screenshot.as_ref().unwrap_or(&vec![]))
        ));
    }

    fn hide_screenshot(&self) {
        self.window.evaluate_script(&format!("hideScreenshot({})", self.slot_index));
    }
}
```

### 7. Stale content handling

```rust
impl WebViewSlot {
    fn mark_content_stale(&mut self) { self.is_content_stale = true; }

    fn on_activate(&mut self) {
        if self.is_content_stale {
            self.reload();
            self.is_content_stale = false;
        }
    }
}
```

### 8. Session integration

```rust
session.register_subsystem(WebViewStack::new(config));

session.on_navigate(|intent, session| {
    match intent.presentation {
        Presentation::Push   => session.stack().push(intent.route, intent.view_kind),
        Presentation::Pop    => session.stack().pop(),
        Presentation::Modal  => session.stack().present_modal(intent.route),
        Presentation::Replace=> session.stack().replace_current(intent.route),
        Presentation::Root   => session.stack().set_root(intent.route),
        _ => { /* handled by session backbone */ }
    }
});
```

## Tasks

### Stack manager
- [ ] Create `src/stack.rs` with `WebViewStack`, `WebViewSlot`, `SlotState`
- [ ] Implement `push()` — screenshot current, move WebView to new route
- [ ] Implement `pop()` — show screenshot instantly, reload WebView behind it
- [ ] Implement `morph()` — in-place content update, no stack change
- [ ] Implement `replace()` — swap current slot, no new stack entry

### Screenshot capture
- [ ] Implement JS canvas-based screenshot capture via `evaluate_script`
- [ ] Implement `show_screenshot()` / `hide_screenshot()` CSS overlay
- [ ] Test: screenshot captured on deactivation, displayed on back navigation

### Stale content
- [ ] Implement `mark_content_stale()` and `on_activate()` reload
- [ ] Test: stale content is reloaded when slot becomes active

### Stack config
- [ ] Implement configurable screenshot limits (max count, memory budget)
- [ ] LRU eviction when limits are hit

### Session integration
- [ ] Register stack as session subsystem
- [ ] Wire navigation decisions (push/pop/morph/replace/root) to stack methods

### Memory management
- [ ] Evict oldest screenshots when memory budget exceeded
- [ ] Test: memory pressure triggers screenshot eviction

## Verification Commands

```bash
cargo test --package foundation_platform -- stack
```
