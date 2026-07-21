---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F06-webview-stack"
this_file: "specifications/52-tauri-foundation-platform/features/F06-webview-stack/feature.md"

status: completed
priority: high
created: 2026-07-17
updated: 2026-07-21

depends_on:
  - "F01-session-backbone"
  - "F02-route-handler"

tasks:
  completed: 7
  uncompleted: 0
  total: 7
  completion_percentage: 100%
---

# F06 — Single-WebView stack manager (v1)

## Overview

Implement Basecamp-model single-WebView + screenshot-swap stack manager. v1
operates with one shared WebView. Screenshot capture on deactivation, instant
screenshot display on back navigation, stale content detection, back-gesture
integration. Multi-WebView (desktop+unstable) is post-MVP.

[Decision 10](../decisions/10-multi-webview-stack.md).

---

## Part A — WebViewSlot and slot state

```rust
// foundation_platform/src/stack.rs

pub struct WebViewSlot {
    pub route: String,
    pub page_identity: Option<PageIdentity>,
    pub webview: Option<Webview<R>>,
    pub screenshot: Option<Vec<u8>>,
    pub state: SlotState,
    pub is_content_stale: bool,
}

impl WebViewSlot {
    pub fn capture_screenshot(&mut self) {
        if let Some(wv) = &self.webview {
            // JS canvas-based capture: evaluate_script("canvas.toDataURL()")
            // Store as base64-encoded PNG
            let script = "document.documentElement.outerHTML"; // placeholder
            // wv.evaluate_script(script) → parse → encode → store
            self.screenshot = Some(vec![]); // placeholder
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState { Screenshot, Preloading, Ready, Active, Transitioning }
```

### A.2 — WebViewStack

```rust
pub struct WebViewStack<R: Runtime> {
    window: WebviewWindow<R>,
    slots: Vec<WebViewSlot>,
    active_index: usize,
    config: StackConfig,
}

pub struct StackConfig {
    pub max_screenshots: usize,         // default: 10
    pub screenshot_memory_budget: usize, // default: 50MB
    /// Single shared WebView for v1 (Basecamp model).
    /// All screens share this — the pool is this one WebView.
    pub shared_webview: Option<Webview<R>>,
}
```

### A.3 — Navigation: push (v1 Basecamp model)

```
1. User taps link → session intercepts → RouteDecision
2. If Presentation::Push:
   - Deactivate current: screenshot captured, slot → Screenshot
   - Move shared WebView to new screen: navigate to new URL
   - foundation-wasm-ui.js re-injected, content loads
   - Animate: old screenshot slides left, new content slides in
3. Previous slot shows its screenshot, has no live WebView.
```

### A.4 — Navigation: pop (v1)

```
1. User taps back → stack manager pops top slot
2. Previous slot has screenshot (Screenshot):
   - Show screenshot INSTANTLY (feels fast, exactly like Basecamp)
   - Move shared WebView to this slot, navigate to route
   - Screenshot → Active once content loads
3. If content is stale (isShowingStaleContent) → reload after visible
```

### A.5 — Screenshot lifecycle

```rust
impl<R: Runtime> WebViewStack<R> {
    /// Push a new screen onto the stack.
    pub fn push(&mut self, route: &str, view_kind: ViewKind) {
        // 1. Capture screenshot of current active slot
        if let Some(slot) = self.slots.get_mut(self.active_index) {
            slot.capture_screenshot();
            slot.state = SlotState::Screenshot;
        }

        // 2. Create new slot, navigate shared WebView to new URL
        let mut new_slot = WebViewSlot::new(route);
        if let Some(wv) = &self.config.shared_webview {
            let _ = wv.navigate(Url::parse(&format!("ewe://localhost{}", route)).unwrap());
            new_slot.webview = Some(/* shared WebView moves here */);
        }
        new_slot.state = SlotState::Active;

        // 3. Add to slots, update active index
        self.slots.push(new_slot);
        self.active_index = self.slots.len() - 1;
    }

    /// Pop the top screen.
    pub fn pop(&mut self) {
        if self.slots.len() <= 1 { return; }

        // Remove top slot
        self.slots.pop();

        // Activate previous slot
        self.active_index = self.slots.len() - 1;
        let prev = &mut self.slots[self.active_index];

        // Show screenshot instantly, reload behind it
        prev.show_screenshot();
        if prev.is_content_stale {
            prev.reload();
            prev.is_content_stale = false;
        }
        prev.state = SlotState::Active;
    }
}
```

### A.5 — Session integration

```rust
// Registered as a session subsystem:
session.register_subsystem(WebViewStack::new(config));

// On navigation decisions, the stack manager reacts:
session.on_navigate(|intent, session| {
    match intent.presentation {
        Presentation::Push => session.stack().push(&intent.url, intent.view_kind),
        Presentation::Pop => session.stack().pop(),
        Presentation::Replace => session.stack().replace_current(&intent.url),
        Presentation::Root => session.stack().set_root(&intent.url),
        _ => { /* Morph, Modal, External handled elsewhere */ }
    }
});
```

---

## Verification

```bash
cargo test --package foundation_platform -- stack
```
