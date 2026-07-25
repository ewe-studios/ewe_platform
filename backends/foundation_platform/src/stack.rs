//! WebView stack manager with multi-WebView pool (F29 Stage 3).
//!
//! v1: Basecamp single-WebView + screenshot model.
//! v2 (F29): Multi-WebView pool — named WebViews for `ViewKind::WebView`
//! routing, background preload, and three presentation modes (Morph,
//! Replace, New).
//!
//! `WebView` operations (navigate, screenshot, eval) are behind a trait
//! so the stack manager can be unit tested without a running Tauri app.

use std::collections::{HashMap, VecDeque};

use foundation_ui_traits::PageIdentity;

// ── WebView trait (testable abstraction) ─────────────────────────────

/// Operations the stack manager needs from a `WebView`.
/// Trait lets us unit-test navigation logic without a running Tauri app.
pub trait WebViewOps: Send + Sync + 'static {
    /// Navigate the `WebView` to a URL.
    fn navigate(&self, url: &str);
    /// Capture a screenshot as PNG bytes.
    fn screenshot(&self) -> Vec<u8>;
    /// Execute JavaScript in the `WebView`.
    fn eval(&self, script: &str);
    /// Reload the current page.
    fn reload(&self);
}

// ── Slot state ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    /// Screenshot-only. No live `WebView`. Shows a frozen image.
    Screenshot,
    /// `WebView` is loading in the background.
    Preloading,
    /// `WebView` is loaded and ready but not visible.
    Ready,
    /// `WebView` is visible and active.
    Active,
    /// `WebView` is being transitioned (animation in progress).
    Transitioning,
}

// ── Slot kind ────────────────────────────────────────────────────────

/// How a slot was pushed onto the stack — determines back-button behavior.
///
/// WHY: Push and Modal both create a new WebView, but back should return to
/// the previous screen on Push and dismiss only the overlay on Modal. Without
/// this tag, `pop()` cannot tell whether it's popping the main stack or just
/// a transient sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotKind {
    /// A full-screen push. Back navigates to the previous slot.
    FullScreen,
    /// A modal overlay. Back closes the modal without changing the underlying
    /// stack. The slot underneath is left exactly as it was.
    Modal,
}

// ── WebView slot ─────────────────────────────────────────────────────

/// A single screen in the navigation stack.
pub struct WebViewSlot {
    pub route: String,
    pub page_identity: Option<PageIdentity>,
    pub screenshot: Option<Vec<u8>>,
    pub state: SlotState,
    pub is_content_stale: bool,
    /// How this slot was pushed — FullScreen or Modal.
    pub kind: SlotKind,
}

impl WebViewSlot {
    #[must_use]
    pub fn new(route: &str) -> Self {
        Self {
            route: route.to_string(),
            page_identity: None,
            screenshot: None,
            state: SlotState::Active,
            is_content_stale: false,
            kind: SlotKind::FullScreen,
        }
    }

    #[must_use]
    pub fn new_modal(route: &str) -> Self {
        Self {
            route: route.to_string(),
            page_identity: None,
            screenshot: None,
            state: SlotState::Active,
            is_content_stale: false,
            kind: SlotKind::Modal,
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

// ── WebView pool (F29 Stage 3) ──────────────────────────────────────

/// State of a WebView in the pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebViewState {
    /// WebView exists but has no content loaded.
    Idle,
    /// WebView is loading content in the background.
    Loading,
    /// WebView has content loaded and is ready (but not visible).
    Ready,
    /// WebView is currently visible and active.
    Active,
    /// WebView is being destroyed.
    Destroying,
}

/// A handle to a WebView in the pool.
#[derive(Debug, Clone)]
pub struct PooledWebView {
    pub label: String,
    pub state: WebViewState,
    pub current_route: Option<String>,
}

impl PooledWebView {
    #[must_use]
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            state: WebViewState::Idle,
            current_route: None,
        }
    }
}

/// Manages a pool of named WebViews for multi-WebView navigation.
///
/// The "main" WebView always exists. Additional WebViews are created
/// on demand when `Presentation::New` or `ViewKind::WebView` directs
/// navigation to a labeled WebView.
#[derive(Debug, Default)]
pub struct WebViewPool {
    /// Label → WebView handle. "main" is always present.
    views: HashMap<String, PooledWebView>,
}

impl WebViewPool {
    #[must_use]
    pub fn new() -> Self {
        let mut views = HashMap::new();
        views.insert("main".to_string(), PooledWebView::new("main"));
        Self { views }
    }

    /// Get or create a WebView by label.
    pub fn get_or_create(&mut self, label: &str) -> &mut PooledWebView {
        self.views
            .entry(label.to_string())
            .or_insert_with(|| PooledWebView::new(label))
    }

    /// Get a WebView by label. Returns `None` if not found.
    #[must_use]
    pub fn get(&self, label: &str) -> Option<&PooledWebView> {
        self.views.get(label)
    }

    /// Return the first idle WebView (for reuse). Excludes "main".
    #[must_use]
    pub fn find_idle(&self) -> Option<&PooledWebView> {
        self.views
            .values()
            .find(|v| v.label != "main" && v.state == WebViewState::Idle)
    }

    /// Mark a WebView's state.
    pub fn set_state(&mut self, label: &str, state: WebViewState) {
        if let Some(view) = self.views.get_mut(label) {
            view.state = state;
        }
    }

    /// Mark a WebView's current route.
    pub fn set_route(&mut self, label: &str, route: &str) {
        if let Some(view) = self.views.get_mut(label) {
            view.current_route = Some(route.to_string());
        }
    }

    /// Number of WebViews in the pool.
    #[must_use]
    pub fn len(&self) -> usize {
        self.views.len()
    }

    /// Whether the pool is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
    }

    /// Remove a WebView from the pool by label (F35).
    pub fn remove(&mut self, label: &str) {
        self.views.remove(label);
    }

    /// All WebView labels currently in the pool (F35 — Hotwire Native).
    #[must_use]
    pub fn labels(&self) -> Vec<String> {
        self.views.keys().cloned().collect()
    }
}

// ── Preload entry ────────────────────────────────────────────────────

/// A route queued for background preload.
#[derive(Debug, Clone)]
pub struct PreloadEntry {
    pub route: String,
    /// Target WebView label for the preload.
    pub target_label: Option<String>,
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

/// Manages a navigation stack of `WebViewSlot`s with a shared `WebView`
/// and an optional multi-WebView pool.
///
/// v1: Basecamp single-WebView + screenshot model.
/// v2 (F29): Multi-WebView pool + background preload.
pub struct WebViewStack {
    slots: Vec<WebViewSlot>,
    active_index: usize,
    config: StackConfig,
    /// Track the total screenshot memory usage.
    screenshot_bytes: usize,
    /// Multi-WebView pool (F29 Stage 3). None = single-WebView mode.
    pool: Option<WebViewPool>,
    /// Routes queued for background preload (F29 Stage 3).
    preload_queue: VecDeque<PreloadEntry>,
}

impl WebViewStack {
    /// Create a new stack in single-WebView mode.
    #[must_use]
    pub fn new(config: StackConfig) -> Self {
        Self {
            slots: Vec::new(),
            active_index: 0,
            config,
            screenshot_bytes: 0,
            pool: None,
            preload_queue: VecDeque::new(),
        }
    }

    /// Create a new stack with multi-WebView pool support.
    #[must_use]
    pub fn with_pool(config: StackConfig) -> Self {
        Self {
            slots: Vec::new(),
            active_index: 0,
            config,
            screenshot_bytes: 0,
            pool: Some(WebViewPool::new()),
            preload_queue: VecDeque::new(),
        }
    }

    /// Enable the multi-WebView pool on an existing stack.
    pub fn enable_pool(&mut self) {
        if self.pool.is_none() {
            self.pool = Some(WebViewPool::new());
        }
    }

    /// Access the WebView pool, if enabled.
    #[must_use]
    pub fn pool(&self) -> Option<&WebViewPool> {
        self.pool.as_ref()
    }

    /// Mutably access the WebView pool.
    pub fn pool_mut(&mut self) -> Option<&mut WebViewPool> {
        self.pool.as_mut()
    }

    /// Initialize the stack with a root route.
    pub fn init(&mut self, route: &str) {
        self.slots.push(WebViewSlot::new(route));
    }

    /// Number of slots in the stack.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.slots.len()
    }

    /// The currently active slot index.
    #[must_use]
    pub fn active(&self) -> usize {
        self.active_index
    }

    /// Get the active slot's route.
    #[must_use]
    pub fn active_route(&self) -> Option<&str> {
        self.slots.get(self.active_index).map(|s| s.route.as_str())
    }

    // ── Navigation: push ──────────────────────────────────────────

    /// Push a new FullScreen slot. Back navigates to the previous slot.
    pub fn push(
        &mut self,
        route: &str,
        webview: &dyn WebViewOps,
    ) {
        if let Some(slot) = self.slots.get_mut(self.active_index) {
            let ss = webview.screenshot();
            slot.set_screenshot(ss);
            slot.state = SlotState::Screenshot;
            self.screenshot_bytes += slot.screenshot.as_ref().map_or(0, std::vec::Vec::len);
        }

        let mut new_slot = WebViewSlot::new(route);
        new_slot.state = SlotState::Active;
        self.slots.push(new_slot);
        self.active_index = self.slots.len() - 1;
        webview.navigate(route);
        self.evict_if_needed();
    }

    /// Push a Modal slot. The slot underneath stays at its route — backing
    /// out of a modal only dismisses the modal, never touches the main stack.
    pub fn push_modal(
        &mut self,
        route: &str,
        webview: &dyn WebViewOps,
    ) {
        if let Some(slot) = self.slots.get_mut(self.active_index) {
            slot.state = SlotState::Ready;
        }

        let mut new_slot = WebViewSlot::new_modal(route);
        new_slot.state = SlotState::Active;
        self.slots.push(new_slot);
        self.active_index = self.slots.len() - 1;
        webview.navigate(route);
    }

    // ── Navigation: pop ───────────────────────────────────────────

    /// Pop the top screen.
    ///
    /// Modal: just drops the modal slot and reactivates the one underneath
    /// without navigating — the underlying WebView is still at its route.
    ///
    /// FullScreen: drops the top slot, shows screenshot if available, then
    /// navigates to the previous route.
    ///
    /// Returns `None` when the stack has only the root slot — back should
    /// exit the app or do nothing at that point.
    pub fn pop(&mut self, webview: &dyn WebViewOps) -> Option<String> {
        self.pop_with(|prev_route, prev| {
            // For FullScreen: screenshot check + navigate + reload if stale
            webview.eval("showScreenshot()");
            webview.navigate(prev_route);
            if prev.is_content_stale {
                webview.reload();
                prev.is_content_stale = false;
            }
        })
    }

    /// Pop the top screen with a custom cleanup callback.
    ///
    /// The closure receives the previous slot's route and a mutable reference
    /// to the previous slot (now the active one after pop). It fires AFTER the
    /// top slot is removed and the previous slot is reactivated. For Modal
    /// slots, only the reactivation happens — the closure is NOT called
    /// (the underlying WebView is still at its content).
    ///
    /// Returns `None` when the stack has only the root slot.
    pub fn pop_with(&mut self, on_pop: impl FnOnce(&str, &mut WebViewSlot)) -> Option<String> {
        if self.slots.len() <= 1 {
            return None;
        }

        let top_kind = self.slots[self.active_index].kind;

        if let Some(top) = self.slots.get(self.active_index) {
            self.screenshot_bytes = self.screenshot_bytes.saturating_sub(
                top.screenshot.as_ref().map_or(0, std::vec::Vec::len),
            );
        }
        self.slots.pop();
        self.active_index = self.slots.len() - 1;

        let prev = &mut self.slots[self.active_index];

        if top_kind == SlotKind::Modal {
            prev.state = SlotState::Active;
            prev.clear_screenshot();
            return Some(format!("dismissed modal → {}", prev.route));
        }

        let route = prev.route.clone();
        prev.state = SlotState::Active;
        on_pop(&route, prev);
        Some(route)
    }

    // ── Navigation: morph ─────────────────────────────────────────

    /// Replace the current screen's content in-place. No stack change.
    /// The `WebView` stays active; only the route changes.
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

    // ── Pool-aware push (F29 Stage 3) ─────────────────────────────

    /// Push a new screen using a named WebView from the pool.
    ///
    /// `Push` creates a FullScreen slot; back navigates to the previous.
    /// `Modal` creates a Modal slot; back dismisses only the overlay.
    /// `Replace` reuses the current WebView (same as `replace()`).
    /// `Morph` updates in-place without stack change.
    /// `Root` clears everything and starts from the named route.
    pub fn push_with_presentation(
        &mut self,
        route: &str,
        presentation: &foundation_ui_traits::Presentation,
        target_label: Option<&str>,
        webview: &dyn WebViewOps,
    ) {
        match presentation {
            foundation_ui_traits::Presentation::Morph => {
                self.morph(route, webview);
            }
            foundation_ui_traits::Presentation::Replace => {
                self.replace(route, webview);
            }
            foundation_ui_traits::Presentation::Push => {
                self.push(route, webview);
                if let Some(pool) = self.pool.as_mut() {
                    let label = target_label
                        .map(String::from)
                        .unwrap_or_else(|| format!("wv_{}", self.slots.len()));
                    pool.get_or_create(&label);
                    pool.set_state(&label, WebViewState::Active);
                    pool.set_route(&label, route);
                }
            }
            foundation_ui_traits::Presentation::Modal => {
                // Modal: hide current, push overlay. Back dismisses
                // only the modal — the slot underneath stays at its route.
                self.push_modal(route, webview);
                if let Some(pool) = self.pool.as_mut() {
                    let label = target_label
                        .map(String::from)
                        .unwrap_or_else(|| format!("wv_{}", self.slots.len()));
                    pool.get_or_create(&label);
                    pool.set_state(&label, WebViewState::Active);
                    pool.set_route(&label, route);
                }
            }
            foundation_ui_traits::Presentation::External => {}
            foundation_ui_traits::Presentation::Root => {
                self.set_root(route, webview);
            }
        }
    }

    // ── Background preload (F29 Stage 3) ───────────────────────────

    /// Enqueue a route for background preload into an idle WebView.
    ///
    /// The preload is drained by calling `drain_preloads()` with a
    /// `WebViewOps` that supports navigation. Preloaded pages are
    /// ready when the user navigates to them.
    pub fn preload(&mut self, route: &str, target_label: Option<&str>) {
        self.preload_queue.push_back(PreloadEntry {
            route: route.to_string(),
            target_label: target_label.map(String::from),
        });
    }

    /// Drain the preload queue, navigating idle WebViews to preloaded
    /// routes. Returns the number of preloads processed.
    ///
    /// Call this after each navigation to keep the preload pipeline full.
    pub fn drain_preloads(&mut self, webview: &dyn WebViewOps) -> usize {
        let mut processed = 0;
        while let Some(entry) = self.preload_queue.pop_front() {
            // In single-WebView mode, skip preloads (would interrupt user)
            if self.pool.is_none() {
                continue;
            }

            // Find an idle WebView
            let idle_label = self
                .pool
                .as_ref()
                .and_then(|p| p.find_idle().map(|v| v.label.clone()));

            let target_label = entry.target_label.clone();
            if let Some(label) = idle_label.or(target_label) {
                if let Some(pool) = self.pool.as_mut() {
                    pool.set_state(&label, WebViewState::Loading);
                    pool.set_route(&label, &entry.route);
                }
                webview.navigate(&entry.route);
                processed += 1;
            } else {
                // No idle WebView — put back and stop
                self.preload_queue.push_front(entry);
                break;
            }
        }
        processed
    }

    /// Number of pending preloads.
    #[must_use]
    pub fn preload_queue_len(&self) -> usize {
        self.preload_queue.len()
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
                slot.screenshot.as_ref().map_or(0, std::vec::Vec::len),
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
                    slot.screenshot.as_ref().map_or(0, std::vec::Vec::len),
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
    #[must_use]
    pub fn screenshot_memory_usage(&self) -> usize {
        self.screenshot_bytes
    }

    /// Get a reference to all slots (for inspection in tests).
    #[must_use]
    pub fn slots(&self) -> &[WebViewSlot] {
        &self.slots
    }

    /// Get a mutable reference to all slots.
    #[must_use]
    pub fn slots_mut(&mut self) -> &mut Vec<WebViewSlot> {
        &mut self.slots
    }

    /// Push a pre-constructed slot onto the stack and make it active.
    /// Used by `execute_decision` when `Presentation::Push` or `Presentation::Modal`.
    pub fn push_slot(&mut self, slot: WebViewSlot) {
        self.slots.push(slot);
        self.active_index = self.slots.len() - 1;
    }

    /// Clear stack and set a new root route (no WebView ops).
    /// Used by `execute_decision` when `Presentation::Root`.
    pub fn set_root_slot(&mut self, route: &str) {
        self.screenshot_bytes = 0;
        self.slots.clear();
        self.slots.push(WebViewSlot::new(route));
        self.active_index = 0;
    }
}

// Tests moved to tests/stack_suite.rs
