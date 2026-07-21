//! Tests for the single-WebView stack manager (v1 — Basecamp model).

use std::sync::Mutex;

use foundation_platform::*;

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
    assert_eq!(wv.reload_count(), 1); // stale -> reload
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

    assert_eq!(stack.screenshot_memory_usage(), 6); // 2 screenshots x 3 bytes
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
        screenshot_memory_budget: 5, // only 5 bytes - 1st screenshot (3 bytes) fits, 2nd (3 bytes) triggers eviction
    };
    let mut stack = WebViewStack::new(config);
    let wv = FakeWebView::new();
    stack.init("/app/home");
    stack.push("/app/s1", &wv); // 3 bytes captured for /app/home
    stack.push("/app/s2", &wv); // 3 bytes captured for /app/s1 -> total 6 > 5 -> evict oldest

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
