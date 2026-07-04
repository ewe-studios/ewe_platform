# 21 — Multi-WebView native-stack simulation: screenshot-swap + background preload

**Date:** 2026-07-04
**Status:** Resolved

### Decision

`foundation_platform` provides a WebView stack manager that enables native-feeling
screen transitions, background preloading, and screenshot-based responsiveness.
It is inspired by Basecamp's shared-WebView + screenshot pattern (verified
against `Session.swift`, `Visit.swift`, `ColdBootVisit.swift`,
`JavaScriptVisit.swift`), but extended for Tauri's multi-WebView capabilities
and the platform's multi-protocol rendering model.

This is v1. The session backbone is designed for multiple WebView contexts from
the start. Single-WebView is the default; multi-WebView is always available.

### What Basecamp does (what we learn from)

Basecamp's approach (verified from source):

- **One `Session` = one `WKWebView`.** A session owns a single WebView. Multiple
  `VisitableViewController`s exist in the navigation stack, but only ONE is
  "active" — it holds the shared WebView in its view hierarchy.
- **Inactive screens show screenshots.** `VisitableViewController` captures a
  snapshot (`updateVisitableScreenshot()`) before the WebView is moved away.
  When the screen is inactive, it displays this screenshot. The user sees a
  frozen image of the last state.
- **WebView moves between screens.** `activateVisitable()` moves the shared
  `WKWebView` into the active view controller's view. `deactivateVisitable()`
  removes it, captures a screenshot, shows the screenshot.
- **Cold boot vs JavaScript visit.** First load is a `ColdBootVisit` (full page
  load via `WKWebView.load(URLRequest)`). After Turbo JS is initialized, all
  subsequent navigations are `JavaScriptVisit` (Turbo handles it via the bridge).
  Cold boot → `webView.load(...)` → `onPageFinished` → Turbo initialized →
  `JavaScriptVisit` from there.
- **Screenshot cache can be cleared.** `Session.clearSnapshotCache()` →
  `bridge.clearSnapshotCache()`. Stale screenshots are discarded. The next time
  the screen appears, it reloads.
- **Two sessions: main + modal.** `Navigator` owns `session` (main stack) and
  `modalSession` (modal stack). Each has its own WebView. Navigation can switch
  between stacks.

### What we build on top (Tauri specifics)

Tauri's model is different from UIKit. Instead of moving a shared WebView between
view controllers, Tauri has:

- **`WebviewWindow`** — a window with an integrated WebView. One WebView per window.
- **Child WebViews** — a window can have child WebViews (`Window::add_child`).
  Each child is a separate `Webview<R>` with its own label.
- **`WebviewManager`** — tracks all WebViews by label. The shell can create,
  destroy, navigate, and evaluate JS in any WebView.
- **Windows are the heavy resource.** Creating a new window is expensive.
  Creating a new child WebView within a window is lighter.

Our stack manager works WITHIN a single Tauri window, managing multiple child
WebViews and screenshot swap:

```
Tauri Window (single OS window)
  └── WebviewManager
        ├── [active]   child WebView "screen-3" (visible, rendering)
        ├── [inactive] child WebView "screen-2" (hidden, showing screenshot)
        ├── [preload]  child WebView "screen-4" (hidden, loading in background)
        └── [pool]     child WebView _idle (warm, ready to be assigned)
```

### The stack manager design

```rust
struct WebViewStack {
    /// The Tauri window these WebViews live in.
    window: WebviewWindow<R>,

    /// All managed slots.
    slots: Vec<WebViewSlot>,

    /// Idle WebViews available for assignment (pool).
    pool: Vec<Webview<R>>,

    /// The slot currently visible.
    active_index: usize,

    /// Maximum concurrent WebViews (configurable).
    max_active: usize,
}

struct WebViewSlot {
    /// The screen this slot represents.
    route: String,
    page_identity: PageIdentity,

    /// The WebView (if active/loaded) or None (if screenshot-only).
    webview: Option<Webview<R>>,

    /// Screenshot of the last rendered state (for inactive slots).
    screenshot: Option<Vec<u8>>,

    /// Current state.
    state: SlotState,
}

enum SlotState {
    /// Screenshot-only. No live WebView. Shows a frozen image.
    Screenshot,
    /// WebView is loading in the background.
    Preloading,
    /// WebView is loaded and ready but not visible.
    Ready,
    /// WebView is visible and active.
    Active,
    /// WebView is being transitioned (morph animation in progress).
    Transitioning,
}
```

### Navigation flow: push (navigate forward)

1. User taps a link on the active screen.
2. Session backbone intercepts → route handler returns `RouteDecision`.
3. If presentation is `Push`:
   - Capture screenshot of current active WebView → store in current slot.
   - Current slot: `Active → Ready` (or `Screenshot` if we move the WebView).
   - Create or assign a WebView for the new screen.
   - New slot: `Active` (or `Preloading` if we want to show a screenshot first).
   - Animate transition (slide from right on mobile, no animation on desktop
     unless configured).
4. New screen renders → user sees the new content.

### Navigation flow: pop (navigate back)

1. User taps back (or native back gesture).
2. Stack manager pops the top slot.
3. If the previous slot has a live WebView (`Ready`):
   - Show the previous WebView → `Ready → Active`.
   - It may briefly show its last rendered state (no reload needed if state
     is still valid).
   - If the content is stale (`isShowingStaleContent` flag), trigger a reload
     after the WebView becomes visible.
4. If the previous slot has only a screenshot (`Screenshot`):
   - Show the screenshot INSTANTLY (feels fast, exactly like Basecamp).
   - Assign a WebView from the pool (or create one).
   - Load the route → `Preloading → Active`.
   - Once loaded, hide the screenshot, show the real content.
   - The user perceived a native-feeling instant back gesture, even though
     the WebView needed to reload.

### Navigation flow: preload (background)

1. The route handler or predictive logic decides "the user will likely tap
   this link next."
2. Stack manager assigns an idle WebView from the pool.
3. Loads the target route in the background → `Preloading`.
4. When the user actually taps:
   - If preload is complete (`Ready`): instant swap. Screenshot → real content
     with no loading delay.
   - If preload is still in progress: show screenshot immediately, swap to
     real content when ready.

### WebView pool

Idle WebViews are kept warm rather than destroyed. Destroying and recreating
WebViews is expensive (process creation, JS engine init, WASM instantiation).
The pool reuses them:

```rust
impl WebViewStack {
    /// Get a WebView from the pool, or create one if the pool is empty
    /// and we haven't hit the max.
    fn acquire_webview(&mut self) -> Option<Webview<R>> {
        self.pool.pop().or_else(|| {
            if self.slots.iter().filter(|s| s.webview.is_some()).count() < self.max_active {
                Some(self.create_webview())
            } else {
                None // pool exhausted, fall back to screenshot-only
            }
        })
    }

    /// Return a WebView to the pool for reuse.
    fn release_webview(&mut self, webview: Webview<R>) {
        // Navigate to about:blank to free memory.
        webview.navigate("about:blank");
        // Reset JS state.
        webview.evaluate_script("globalThis.__platformSession = null;");
        // Keep it warm in the pool.
        self.pool.push(webview);
    }
}
```

### Screenshot capture

```rust
impl WebViewSlot {
    fn capture_screenshot(&mut self) {
        if let Some(webview) = &self.webview {
            // Tauri's WebView API: capture a screenshot.
            // Falls back to JS canvas-based capture if native API unavailable.
            self.screenshot = Some(webview.screenshot().unwrap_or_default());
        }
    }

    fn show_screenshot(&self) {
        // Display the screenshot as an <img> over the slot's area.
        // CSS: position absolute, z-index above WebView, fade in.
        // The WebView is hidden behind the screenshot.
        self.window.evaluate_script(&format!(
            "showScreenshot({}, '{}')",
            self.slot_index,
            base64::encode(self.screenshot.as_ref().unwrap_or(&vec![]))
        ));
    }

    fn hide_screenshot(&self) {
        // Fade out the screenshot <img>, reveal the live WebView behind it.
        self.window.evaluate_script(&format!(
            "hideScreenshot({})", self.slot_index
        ));
    }
}
```

### Stale content handling

Basecamp's `Session.markContentAsStale()` and `markSnapshotCacheAsStale()`:

```rust
impl WebViewSlot {
    /// Mark that this slot's content is stale — reload on next activation.
    fn mark_content_stale(&mut self) {
        self.is_content_stale = true;
    }

    /// Mark that the screenshot is stale — clear it, show activity indicator.
    fn mark_screenshot_stale(&mut self) {
        self.screenshot = None;
        self.is_screenshot_stale = true;
    }

    /// Called when the slot becomes active.
    fn on_activate(&mut self) {
        if self.is_content_stale {
            // Reload the route. Show the screenshot (if any) during reload.
            self.reload();
            self.is_content_stale = false;
        }
        if self.is_screenshot_stale {
            // No screenshot to show. Clear and show activity indicator.
            self.clear_screenshot();
            self.show_activity_indicator();
            self.is_screenshot_stale = false;
        }
    }
}
```

### Back gesture integration

On mobile, the native back gesture (swipe from left edge on iOS, back button/gesture
on Android) triggers a stack pop. The platform integrates:

- **iOS:** `UINavigationController` interactive pop gesture. The platform's
  `VisitableViewController` (or Tauri equivalent) intercepts the gesture and
  calls `stack.pop()`.
- **Android:** `OnBackPressedCallback` or the navigation fragment's back stack.
  The platform's fragment intercepts and calls `stack.pop()`.
- **Desktop:** No native back gesture. Browser back button navigates within the
  WebView's history. The session backbone intercepts and can map to stack pop.

### When to use multiple WebViews vs screenshots

| Scenario | Strategy | Why |
|---|---|---|
| Navigating forward to a new screen | Screenshot current + WebView for new screen (if available) | Instant transition. New screen loads in its own WebView. |
| Navigating back to a recent screen | Show the existing WebView (if still in pool) | No reload needed. State preserved. |
| Navigating back to an old screen | Screenshot first, assign WebView, reload | Screenshot gives instant feedback. WebView loads fresh. |
| Preloading a likely next screen | Assign idle WebView, load in background | Zero wait when user taps. |
| Content-heavy screen (maps, video) | Keep its WebView alive longer | Don't recycle a WebView that's expensive to recreate. |
| Simple static content | Screenshot-only, no WebView allocated | Saves memory. Reloads cheaply when needed. |
| Modal presentation | Separate WebView (like Basecamp's modalSession) | Modal has independent navigation. Don't mix with main stack. |

### Memory management

WebViews are expensive. The stack manager enforces limits:

```rust
struct StackConfig {
    /// Maximum WebViews alive at once (default: 3).
    max_active_webviews: usize,
    /// Maximum screenshots cached (default: 10).
    max_screenshots: usize,
    /// Maximum idle WebViews in the pool (default: 1).
    max_pool_size: usize,
    /// How long an idle WebView stays warm before release (default: 60s).
    pool_ttl: Duration,
    /// Maximum memory budget for screenshots (default: 50MB).
    screenshot_memory_budget: usize,
}
```

When limits are hit:
- Oldest screenshots are evicted (LRU).
- Idle WebViews past TTL are destroyed (not returned to pool).
- New WebView requests when pool is exhausted → screenshot-only fallback.

### Tauri integration

| Tauri primitive | How the stack manager hooks in |
|---|---|
| `WebviewWindow` | The stack manager operates within a single window. It creates child WebViews via `Window::add_child`. |
| Child `Webview<R>` | Each slot's WebView is a Tauri child WebView with a unique label. The stack manager positions, shows, and hides them. |
| `Webview::set_position` / `set_size` | Child WebViews are positioned within the window. The active one fills the content area. Inactive ones are moved off-screen or hidden. |
| `Webview::set_visible` / `hide` | Only the active WebView is visible. Others are hidden (screenshot shown instead). |
| `Webview::navigate` | Preloading navigates to the target URL. Screenshot-only slots navigate when activated. |
| `Webview::evaluate_script` | JS calls for showing/hiding screenshots, communicating session identity, and triggering reloads. |
| `Webview::screenshot` (or JS canvas fallback) | Captured when a slot is deactivated. Stored in the slot's state. |
| `on_navigation` callback | Intercepted by the session backbone for route policy. The stack manager reacts to push/pop decisions. |
| Mobile lifecycle events | On `didEnterBackground`: cache screenshots, release idle pool. On `willEnterForeground`: validate stale content, re-warm pool if needed. |

### Integration with the session backbone

The stack manager is a subsystem registered with the session backbone:

```rust
session.register_subsystem(WebViewStack::new(config));

// On navigation:
session.on_navigate(|intent, session| {
    match intent.presentation {
        Presentation::Push => {
            session.stack().push(intent.route, intent.render_mode);
        }
        Presentation::Pop => {
            session.stack().pop();
        }
        Presentation::Modal => {
            session.stack().present_modal(intent.route);
        }
        Presentation::Replace => {
            session.stack().replace_current(intent.route);
        }
        _ => { /* handled by session backbone */ }
    }
    Some(RouteDecision::default())
});
```

### How this differs from Basecamp

| Aspect | Basecamp | foundation_platform |
|---|---|---|
| WebViews | 1 per Session (shared) | Multiple child WebViews (pool + preload + active) |
| Screenshot lifecycle | Manual (`cacheSnapshot`, `clearSnapshotCache`) | Automatic (captured on deactivation, evicted on memory pressure) |
| Preloading | No (loads on demand) | Yes (predictive + explicit) |
| Pooling | No (single WebView reused by moving) | Yes (WebView pool with TTL and size limits) |
| Stale detection | Manual (`markContentAsStale`, `markSnapshotCacheAsStale`) | Automatic (page identity change, server invalidation event) |
| Platform | UIKit/Android fragments | Tauri (cross-platform, single Rust API) |
| Multi-window | UINavigationController manages stack | Tauri `WebviewWindow` + child WebViews |
| Render modes | HTML only (Turbo) | Any protocol (HTML, DomOps, Arrow IPC) through rendering lane |
