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

// Tests moved to tests/stack_suite.rs
