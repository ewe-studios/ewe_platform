//! Live CDP smoke test against a real Chromium (spec-43 phase-1 milestone 2).
//!
//! Gated behind `--features browser-tests` so CI without a browser skips it.
//! Validates the whole premise: our pure-Rust WebSocket client speaks CDP to a
//! real browser — launch, navigate, evaluate, screenshot.

#![cfg(feature = "browser-tests")]

use foundation_browser::{BrowserDriver, LaunchConfig};

#[test]
fn launch_navigate_eval_screenshot() {
    let driver = BrowserDriver::launch(LaunchConfig::chromium()).expect("launch chromium");
    let page = driver.new_page().expect("attach a page");

    page.goto("data:text/html,<h1 id='hi'>hello browser</h1>").expect("navigate");

    let text = page
        .eval("document.getElementById('hi').textContent")
        .expect("evaluate");
    assert_eq!(text.as_str(), Some("hello browser"), "DOM read over CDP");

    let out = std::path::Path::new("target/primal-test/smoke.png");
    page.screenshot(out).expect("screenshot");
    let meta = std::fs::metadata(out).expect("screenshot file written");
    assert!(meta.len() > 100, "screenshot should be a non-trivial PNG ({} bytes)", meta.len());
}
