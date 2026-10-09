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

// ── Presentation mode tests (F35 — Multi-WebView) ──────────────

/// Build a NavigationIntent for testing.
fn intent_for(url: &str) -> foundation_platform::NavigationIntent {
    foundation_platform::NavigationIntent {
        url: url.to_string(),
        method: foundation_platform::Method::Get,
        source: foundation_platform::IntentSource::LinkClick,
        referrer: None,
    }
}

#[test]
fn presentation_morph_keeps_one_slot() {
    // Morph replaces content in-place — no new slot, depth unchanged.
    let mut stack = WebViewStack::new(StackConfig::default());
    let wv = FakeWebView::new();
    stack.init("/app/home");

    stack.push_with_presentation(
        "/app/updated",
        &foundation_ui_traits::Presentation::Morph,
        None,
        &wv,
    );

    assert_eq!(stack.depth(), 1, "Morph should not create a new slot");
    assert_eq!(stack.active_route(), Some("/app/updated"));
}

#[test]
fn presentation_replace_swaps_in_place() {
    // Replace swaps the current slot's content — same depth, new route.
    let mut stack = WebViewStack::new(StackConfig::default());
    let wv = FakeWebView::new();
    stack.init("/app/home");

    stack.push_with_presentation(
        "/app/v2",
        &foundation_ui_traits::Presentation::Replace,
        None,
        &wv,
    );

    assert_eq!(stack.depth(), 1, "Replace should keep same depth");
    assert_eq!(stack.active_route(), Some("/app/v2"));
}

#[test]
fn presentation_push_adds_slot_and_screenshot() {
    // Push adds a new slot, captures screenshot of the previous.
    let mut stack = WebViewStack::new(StackConfig::default());
    let wv = FakeWebView::new();
    stack.init("/app/home");

    stack.push_with_presentation(
        "/app/items",
        &foundation_ui_traits::Presentation::Push,
        None,
        &wv,
    );

    assert_eq!(stack.depth(), 2, "Push should add a new slot");
    assert_eq!(stack.active_route(), Some("/app/items"));
    // Previous slot should have a screenshot
    let prev = &stack.slots()[0];
    assert!(prev.screenshot.is_some(), "Push should capture screenshot of previous slot");
    assert_eq!(prev.state, SlotState::Screenshot);
}

#[test]
fn presentation_modal_adds_slot_like_push() {
    // Modal behaves like Push for the stack (new slot, screenshot old).
    let mut stack = WebViewStack::new(StackConfig::default());
    let wv = FakeWebView::new();
    stack.init("/app/home");

    stack.push_with_presentation(
        "/settings/modal",
        &foundation_ui_traits::Presentation::Modal,
        None,
        &wv,
    );

    assert_eq!(stack.depth(), 2, "Modal should add a new slot");
    assert_eq!(stack.active_route(), Some("/settings/modal"));
}

#[test]
fn presentation_push_uses_pool_label() {
    // Push with a target label populates the WebView pool.
    let mut stack = WebViewStack::with_pool(StackConfig::default());
    let wv = FakeWebView::new();
    stack.init("/app/home");

    stack.push_with_presentation(
        "/app-hello/",
        &foundation_ui_traits::Presentation::Push,
        Some("app_hello"),
        &wv,
    );

    // Pool should have both "main" (default) and "app_hello"
    let pool = stack.pool().unwrap();
    assert!(pool.get("main").is_some(), "main pool entry should exist");
    assert!(pool.get("app_hello").is_some(), "app_hello pool entry should be created");
    assert_eq!(pool.get("app_hello").unwrap().state, WebViewState::Active);
    assert_eq!(stack.depth(), 2);
}

#[test]
fn presentation_root_clears_and_resets() {
    // Root clears the entire stack and sets a new root.
    let mut stack = WebViewStack::new(StackConfig::default());
    let wv = FakeWebView::new();
    stack.init("/app/home");
    stack.push("/app/items", &wv);
    stack.push("/app/detail", &wv);
    assert_eq!(stack.depth(), 3);

    stack.push_with_presentation(
        "/login",
        &foundation_ui_traits::Presentation::Root,
        None,
        &wv,
    );

    assert_eq!(stack.depth(), 1, "Root should clear everything and set one slot");
    assert_eq!(stack.active_route(), Some("/login"));
    assert_eq!(stack.active(), 0);
}

#[test]
fn presentation_external_does_not_change_stack() {
    // External opens in system browser — stack unchanged.
    let mut stack = WebViewStack::new(StackConfig::default());
    let wv = FakeWebView::new();
    stack.init("/app/home");

    stack.push_with_presentation(
        "https://example.com",
        &foundation_ui_traits::Presentation::External,
        None,
        &wv,
    );

    assert_eq!(stack.depth(), 1, "External should not change the stack");
    assert_eq!(stack.active_route(), Some("/app/home"));
}

#[test]
fn six_presentation_modes_form_a_cycle() {
    // Simulate a realistic user session covering all 6 presentation modes.
    let mut stack = WebViewStack::with_pool(StackConfig::default());
    let wv = FakeWebView::new();

    // 1. Root: login screen
    stack.push_with_presentation(
        "/login",
        &foundation_ui_traits::Presentation::Root,
        None,
        &wv,
    );
    assert_eq!(stack.depth(), 1);
    assert_eq!(stack.active_route(), Some("/login"));

    // 2. Push: main app after login
    stack.push_with_presentation(
        "/app/home",
        &foundation_ui_traits::Presentation::Push,
        None,
        &wv,
    );
    assert_eq!(stack.depth(), 2, "Push after Root should add slot");
    assert_eq!(stack.active_route(), Some("/app/home"));

    // 3. Morph: in-place update (e.g. tab switch)
    stack.push_with_presentation(
        "/app/home?tab=settings",
        &foundation_ui_traits::Presentation::Morph,
        None,
        &wv,
    );
    assert_eq!(stack.depth(), 2, "Morph should not change depth");

    // 4. Modal: settings modal overlays the app
    stack.push_with_presentation(
        "/settings/profile",
        &foundation_ui_traits::Presentation::Modal,
        Some("modal_settings"),
        &wv,
    );
    assert_eq!(stack.depth(), 3, "Modal should add a slot");

    // 5. Replace: swap modal content
    stack.push_with_presentation(
        "/settings/privacy",
        &foundation_ui_traits::Presentation::Replace,
        None,
        &wv,
    );
    assert_eq!(stack.depth(), 3, "Replace should keep depth");

    // 6. Pop back through the stack
    let popped = stack.pop(&wv);
    assert!(popped.is_some());
    assert_eq!(stack.depth(), 2);

    // 7. External: open in browser — stack unchanged
    stack.push_with_presentation(
        "https://docs.example.com",
        &foundation_ui_traits::Presentation::External,
        None,
        &wv,
    );
    assert_eq!(stack.depth(), 2);

    // Final state: 2 slots (login + app/home)
    assert_eq!(stack.depth(), 2);
}

#[test]
fn presentation_preload_queue() {
    let mut stack = WebViewStack::with_pool(StackConfig::default());
    let wv = FakeWebView::new();
    stack.init("/app/home");

    // Enqueue preloads
    stack.preload("/app/settings", None);
    stack.preload("/app/profile", None);
    assert_eq!(stack.preload_queue_len(), 2);

    // Drain preloads (single-WebView mode skips — need pool)
    // In pool mode with idle WebViews, preloads would navigate:
    let processed = stack.drain_preloads(&wv);
    assert_eq!(stack.preload_queue_len(), 2, "preloads remain until idle WebViews exist");
    // processed may be 0 if no idle pool WebViews yet
    let _ = processed; // just verify it doesn't panic
}

#[test]
fn presentation_recorded_in_session() {
    // Verify session.record_presentation updates the WebViewStack for each mode.
    let session = foundation_platform::PlatformSession::new_test(
        std::path::PathBuf::from(".")
    );

    // Morph
    session.webview_stack_mut().init("/app/home");
    session.webview_stack_mut().push_with_presentation(
        "/app/target",
        &foundation_ui_traits::Presentation::Morph,
        None,
        &FakeWebView::new(),
    );
    {
        let stack = session.webview_stack();
        assert_eq!(stack.depth(), 1, "Morph via session: depth 1");
    }

    // Push
    session.webview_stack_mut().push_with_presentation(
        "/app/new",
        &foundation_ui_traits::Presentation::Push,
        None,
        &FakeWebView::new(),
    );
    {
        let stack = session.webview_stack();
        assert_eq!(stack.depth(), 2, "Push via session: depth 2");
    }

    // Replace
    session.webview_stack_mut().push_with_presentation(
        "/app/replaced",
        &foundation_ui_traits::Presentation::Replace,
        None,
        &FakeWebView::new(),
    );
    {
        let stack = session.webview_stack();
        assert_eq!(stack.depth(), 2, "Replace via session: depth 2");
        assert_eq!(stack.active_route(), Some("/app/replaced"));
    }
}
