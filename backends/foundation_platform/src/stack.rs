//! Single-WebView stack manager (v1 — Basecamp model).
//!
//! One shared WebView per window. Screenshots captured on deactivation,
//! displayed instantly on back navigation. Navigation flows: push, pop,
//! morph, replace, root.
//!
//! Multi-WebView (desktop+unstable) is post-MVP per decision 10.
//!
//! WebView operations (navigate, screenshot, eval) are behind a trait
//! so the stack manager can be unit tested without a running Tauri app.

use foundation_ui_traits::*;

// ── WebView trait (testable abstraction) ─────────────────────────────

/// Operations the stack manager needs from a WebView.
/// Trait lets us unit-test navigation logic without a running Tauri app.
pub trait WebViewOps: Send + Sync + 'static {
    /// Navigate the WebView to a URL.
    fn navigate(&self, url: &str);
    /// Capture a screenshot as PNG bytes.
    fn screenshot(&self) -> Vec<u8>;
    /// Execute JavaScript in the WebView.
    fn eval(&self, script: &str);
    /// Reload the current page.
    fn reload(&self);
}

// ── Slot state ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    /// Screenshot-only. No live WebView. Shows a frozen image.
    Screenshot,
    /// WebView is loading in the background.
    Preloading,
    /// WebView is loaded and ready but not visible.
    Ready,
    /// WebView is visible and active.
    Active,
    /// WebView is being transitioned (animation in progress).
    Transitioning,
}

// ── WebView slot ─────────────────────────────────────────────────────

/// A single screen in the navigation stack.
pub struct WebViewSlot {
    pub route: String,
    pub page_identity: Option<PageIdentity>,
    pub screenshot: Option<Vec<u8>>,
    pub state: SlotState,
    pub is_content_stale: bool,
}

impl WebViewSlot {
    pub fn new(route: &str) -> Self {
        Self {
            route: route.to_string(),
            page_identity: None,
            screenshot: None,
            state: SlotState::Active,
            is_content_stale: false,
        }
    }

    /// Capture a screenshot. In production, calls `WebViewOps::screenshot()`.
    /// In the stack manager, screenshots are captured through the `WebViewStack`.
    pub fn set_screenshot(&mut self, data: Vec<u8>) {
        self.screenshot = Some(data);
    }

    pub fn clear_screenshot(&mut self) {
        self.screenshot = None;
    }

    pub fn mark_stale(&mut self) {
        self.is_content_stale = true;
    }
}

// ── Stack config ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct StackConfig {
    pub max_screenshots: usize,
    pub screenshot_memory_budget: usize,
}

impl Default for StackConfig {
    fn default() -> Self {
        Self {
            max_screenshots: 10,
            screenshot_memory_budget: 50 * 1024 * 1024, // 50MB
        }
    }
}

// ── WebView stack ────────────────────────────────────────────────────

/// Manages a navigation stack of `WebViewSlot`s with a shared WebView.
/// v1: Basecamp single-WebView + screenshot model.
pub struct WebViewStack {
    slots: Vec<WebViewSlot>,
    active_index: usize,
    config: StackConfig,
    /// Track the total screenshot memory usage.
    screenshot_bytes: usize,
}

impl WebViewStack {
    pub fn new(config: StackConfig) -> Self {
        Self {
            slots: Vec::new(),
            active_index: 0,
            config,
            screenshot_bytes: 0,
        }
    }

    /// Initialize the stack with a root route.
    pub fn init(&mut self, route: &str) {
        self.slots.push(WebViewSlot::new(route));
    }

    /// Number of slots in the stack.
    pub fn depth(&self) -> usize {
        self.slots.len()
    }

    /// The currently active slot index.
    pub fn active(&self) -> usize {
        self.active_index
    }

    /// Get the active slot's route.
    pub fn active_route(&self) -> Option<&str> {
        self.slots.get(self.active_index).map(|s| s.route.as_str())
    }

    // ── Navigation: push ──────────────────────────────────────────

    /// Push a new screen onto the stack (Basecamp model).
    ///
    /// 1. Capture screenshot of current active screen
    /// 2. Deactivate current: state → Screenshot
    /// 3. Create new slot, set Active
    /// 4. Navigate shared WebView to new route
    pub fn push(
        &mut self,
        route: &str,
        webview: &dyn WebViewOps,
    ) {
        // 1. Capture screenshot of current active
        if let Some(slot) = self.slots.get_mut(self.active_index) {
            let ss = webview.screenshot();
            slot.set_screenshot(ss);
            slot.state = SlotState::Screenshot;
            self.screenshot_bytes += slot.screenshot.as_ref().map_or(0, |s| s.len());
        }

        // 2. Create new slot
        let mut new_slot = WebViewSlot::new(route);
        new_slot.state = SlotState::Active;
        self.slots.push(new_slot);
        self.active_index = self.slots.len() - 1;

        // 3. Navigate WebView to new route
        webview.navigate(route);

        // 4. Evict old screenshots if over budget
        self.evict_if_needed();
    }

    // ── Navigation: pop ───────────────────────────────────────────

    /// Pop the top screen and navigate back (Basecamp model).
    ///
    /// 1. Drop the top slot
    /// 2. Previous slot has a screenshot → show it instantly
    /// 3. Navigate WebView to the previous route
    /// 4. If content is stale → reload after visible
    pub fn pop(&mut self, webview: &dyn WebViewOps) -> Option<String> {
        if self.slots.len() <= 1 {
            return None; // can't pop the root
        }

        // Drop top slot, free its screenshot memory
        if let Some(top) = self.slots.get(self.active_index) {
            self.screenshot_bytes = self.screenshot_bytes.saturating_sub(
                top.screenshot.as_ref().map_or(0, |s| s.len()),
            );
        }
        self.slots.pop();
        self.active_index = self.slots.len() - 1;

        let prev = &mut self.slots[self.active_index];

        // Show screenshot instantly if we have one
        if prev.screenshot.is_some() {
            webview.eval("showScreenshot()");
        }

        // Navigate to the route
        let route = prev.route.clone();
        webview.navigate(&route);

        // Check stale content
        if prev.is_content_stale {
            webview.reload();
            prev.is_content_stale = false;
        }

        prev.state = SlotState::Active;
        Some(route)
    }

    // ── Navigation: morph ─────────────────────────────────────────

    /// Replace the current screen's content in-place. No stack change.
    /// The WebView stays active; only the route changes.
    pub fn morph(&mut self, route: &str, webview: &dyn WebViewOps) {
        if let Some(slot) = self.slots.get_mut(self.active_index) {
            slot.route = route.to_string();
        }
        webview.navigate(route);
    }

    // ── Navigation: replace ───────────────────────────────────────

    /// Replace the current screen. Same stack depth, new content.
    /// Back button goes to the screen BEFORE the replaced one.
    pub fn replace(&mut self, route: &str, webview: &dyn WebViewOps) {
        if let Some(slot) = self.slots.get_mut(self.active_index) {
            // Capture screenshot for back transition
            let ss = webview.screenshot();
            slot.set_screenshot(ss);
            slot.route = route.to_string();
            slot.is_content_stale = false;
        }
        webview.navigate(route);
    }

    // ── Navigation: root ──────────────────────────────────────────

    /// Clear the entire stack and set a new root. Back button disabled.
    pub fn set_root(&mut self, route: &str, webview: &dyn WebViewOps) {
        self.screenshot_bytes = 0;
        self.slots.clear();
        let slot = WebViewSlot::new(route);
        self.slots.push(slot);
        self.active_index = 0;
        webview.navigate(route);
    }

    // ── Stale content ─────────────────────────────────────────────

    /// Mark the content of a specific slot as stale.
    /// On next activation, it will reload.
    pub fn mark_stale(&mut self, index: usize) {
        if let Some(slot) = self.slots.get_mut(index) {
            slot.mark_stale();
        }
    }

    /// Mark a specific screenshot as stale — clear it.
    pub fn mark_screenshot_stale(&mut self, index: usize) {
        if let Some(slot) = self.slots.get_mut(index) {
            self.screenshot_bytes = self.screenshot_bytes.saturating_sub(
                slot.screenshot.as_ref().map_or(0, |s| s.len()),
            );
            slot.clear_screenshot();
        }
    }

    // ── Screenshot memory management ──────────────────────────────

    fn evict_if_needed(&mut self) {
        while self.screenshot_bytes > self.config.screenshot_memory_budget
            || self.slot_with_screenshots() > self.config.max_screenshots
        {
            // Evict oldest screenshot (LRU: first slot with a screenshot)
            let evicted = self.slots.iter_mut()
                .find(|s| s.screenshot.is_some());
            if let Some(slot) = evicted {
                self.screenshot_bytes = self.screenshot_bytes.saturating_sub(
                    slot.screenshot.as_ref().map_or(0, |s| s.len()),
                );
                slot.clear_screenshot();
            } else {
                break;
            }
        }
    }

    fn slot_with_screenshots(&self) -> usize {
        self.slots.iter().filter(|s| s.screenshot.is_some()).count()
    }

    /// Return total screenshot memory usage in bytes.
    pub fn screenshot_memory_usage(&self) -> usize {
        self.screenshot_bytes
    }

    /// Get a reference to all slots (for inspection in tests).
    pub fn slots(&self) -> &[WebViewSlot] {
        &self.slots
    }
}

// ── Tests ────────────────────────────────────────────────────────────

// NOTE: Tests kept inline because: Tests WebViewStack push/pop/morph navigation, screenshot lifecycle—public API, kept inline for convenience.
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A fake WebView for testing — records operations for assertions.
    struct FakeWebView {
        navigated_urls: Mutex<Vec<String>>,
        eval_scripts: Mutex<Vec<String>>,
        reloads: Mutex<usize>,
        screenshot_data: Vec<u8>,
    }

    impl FakeWebView {
        fn new() -> Self {
            Self {
                navigated_urls: Mutex::new(Vec::new()),
                eval_scripts: Mutex::new(Vec::new()),
                reloads: Mutex::new(0),
                screenshot_data: vec![0x00, 0x01, 0x02], // 3 bytes
            }
        }

        fn navigations(&self) -> Vec<String> {
            self.navigated_urls.lock().unwrap().clone()
        }

        fn reload_count(&self) -> usize {
            *self.reloads.lock().unwrap()
        }

        fn scripts(&self) -> Vec<String> {
            self.eval_scripts.lock().unwrap().clone()
        }
    }

    impl WebViewOps for FakeWebView {
        fn navigate(&self, url: &str) {
            self.navigated_urls.lock().unwrap().push(url.to_string());
        }
        fn screenshot(&self) -> Vec<u8> {
            self.screenshot_data.clone()
        }
        fn eval(&self, script: &str) {
            self.eval_scripts.lock().unwrap().push(script.to_string());
        }
        fn reload(&self) {
            *self.reloads.lock().unwrap() += 1;
        }
    }

    // ── Initialization ────────────────────────────────────────────

    #[test]
    fn stack_starts_empty() {
        let stack = WebViewStack::new(StackConfig::default());
        assert_eq!(stack.depth(), 0);
    }

    #[test]
    fn init_adds_root() {
        let mut stack = WebViewStack::new(StackConfig::default());
        stack.init("/app/home");
        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.active_route(), Some("/app/home"));
    }

    // ── Push ─────────────────────────────────────────────────────

    #[test]
    fn push_adds_new_slot() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();
        stack.init("/app/home");
        stack.push("/app/items", &wv);

        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.active_route(), Some("/app/items"));
        assert_eq!(wv.navigations().last().unwrap(), "/app/items");
    }

    #[test]
    fn push_captures_screenshot_of_previous() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();
        stack.init("/app/home");
        stack.push("/app/items", &wv);

        let prev = &stack.slots()[0];
        assert!(prev.screenshot.is_some());
        assert_eq!(prev.state, SlotState::Screenshot);
    }

    #[test]
    fn push_increments_active_index() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();
        stack.init("/app/home");
        stack.push("/app/items", &wv);
        stack.push("/app/detail", &wv);

        assert_eq!(stack.depth(), 3);
        assert_eq!(stack.active(), 2);
    }

    // ── Pop ──────────────────────────────────────────────────────

    #[test]
    fn pop_returns_to_previous() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();
        stack.init("/app/home");
        stack.push("/app/items", &wv);

        let popped = stack.pop(&wv);
        assert_eq!(popped, Some("/app/home".to_string()));
        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.active_route(), Some("/app/home"));
    }

    #[test]
    fn pop_on_root_returns_none() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();
        stack.init("/app/home");

        assert_eq!(stack.pop(&wv), None);
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn pop_shows_screenshot_instantly() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();
        stack.init("/app/home");
        stack.push("/app/items", &wv);

        let _ = stack.pop(&wv);
        // Screenshot should be shown via JS eval
        assert!(wv.scripts().contains(&"showScreenshot()".to_string()));
    }

    #[test]
    fn pop_reloads_stale_content() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();
        stack.init("/app/home");
        stack.push("/app/items", &wv);

        // Mark home as stale
        stack.mark_stale(0);

        let _ = stack.pop(&wv);
        assert_eq!(wv.reload_count(), 1); // stale → reload
    }

    #[test]
    fn pop_navigates_to_previous_route() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();
        stack.init("/app/home");
        stack.push("/app/items", &wv);

        let _ = stack.pop(&wv);
        let navs = wv.navigations();
        // The last navigation should be to /app/home
        assert!(navs.iter().any(|u| u == "/app/home"));
    }

    // ── Morph ────────────────────────────────────────────────────

    #[test]
    fn morph_replaces_route_in_place() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();
        stack.init("/app/home");
        stack.morph("/app/updated", &wv);

        assert_eq!(stack.depth(), 1); // no new slot
        assert_eq!(stack.active_route(), Some("/app/updated"));
    }

    // ── Replace ──────────────────────────────────────────────────

    #[test]
    fn replace_swaps_current_slot() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();
        stack.init("/app/home");
        stack.replace("/app/new", &wv);

        assert_eq!(stack.depth(), 1); // same depth
        assert_eq!(stack.active_route(), Some("/app/new"));
    }

    // ── Root ─────────────────────────────────────────────────────

    #[test]
    fn set_root_clears_stack() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();
        stack.init("/app/home");
        stack.push("/app/items", &wv);
        stack.push("/app/detail", &wv);

        stack.set_root("/login", &wv);
        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.active_route(), Some("/login"));
        assert_eq!(stack.active(), 0);
    }

    // ── Stale content ────────────────────────────────────────────

    #[test]
    fn mark_stale_triggers_reload_on_activation() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();
        stack.init("/app/home");
        stack.push("/app/items", &wv);
        stack.mark_stale(0);

        let _ = stack.pop(&wv);
        assert_eq!(wv.reload_count(), 1);
    }

    // ── Screenshot memory ────────────────────────────────────────

    #[test]
    fn screenshot_memory_tracked() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();
        stack.init("/app/home");
        stack.push("/app/items", &wv); // captures 3-byte screenshot
        stack.push("/app/detail", &wv); // captures another 3-byte screenshot

        assert_eq!(stack.screenshot_memory_usage(), 6); // 2 screenshots × 3 bytes
    }

    #[test]
    fn pop_frees_screenshot_memory() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();
        stack.init("/app/home");
        stack.push("/app/items", &wv); // captures 3 bytes for /app/home

        let before = stack.screenshot_memory_usage();
        let _ = stack.pop(&wv); // frees /app/items' screenshot if it had one

        // After pop: /app/home still has its screenshot
        assert_eq!(stack.depth(), 1);
        // The screenshot budget accounts for all slots with screenshots
        assert!(stack.screenshot_memory_usage() <= before);
    }

    #[test]
    fn screenshot_eviction_on_budget() {
        let config = StackConfig {
            max_screenshots: 2,
            screenshot_memory_budget: 5, // only 5 bytes — 1st screenshot (3 bytes) fits, 2nd (3 bytes) triggers eviction
        };
        let mut stack = WebViewStack::new(config);
        let wv = FakeWebView::new();
        stack.init("/app/home");
        stack.push("/app/s1", &wv); // 3 bytes captured for /app/home
        stack.push("/app/s2", &wv); // 3 bytes captured for /app/s1 → total 6 > 5 → evict oldest

        assert!(stack.screenshot_memory_usage() <= 5);
    }

    // ── Multi-screen navigation ──────────────────────────────────

    #[test]
    fn full_navigation_cycle() {
        let mut stack = WebViewStack::new(StackConfig::default());
        let wv = FakeWebView::new();

        // Start at home
        stack.init("/app/home");
        assert_eq!(stack.depth(), 1);

        // Push items
        stack.push("/app/items", &wv);
        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.active_route(), Some("/app/items"));

        // Morph to filtered view
        stack.morph("/app/items?filter=recent", &wv);
        assert_eq!(stack.depth(), 2); // still 2
        assert_eq!(stack.active_route(), Some("/app/items?filter=recent"));

        // Push detail
        stack.push("/app/items/42", &wv);
        assert_eq!(stack.depth(), 3);

        // Pop back to items
        let back = stack.pop(&wv);
        assert!(back.is_some());
        assert_eq!(stack.depth(), 2);

        // Pop back to home
        let back = stack.pop(&wv);
        assert_eq!(back, Some("/app/home".to_string()));
        assert_eq!(stack.depth(), 1);

        // Can't pop root
        assert_eq!(stack.pop(&wv), None);
    }
}
