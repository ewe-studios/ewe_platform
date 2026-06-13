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

#[test]
fn locator_geometry_attributes_style_and_input() {
    let driver = BrowserDriver::launch(LaunchConfig::chromium()).expect("launch chromium");
    let page = driver.new_page().expect("attach a page");
    page.goto(
        "data:text/html,\
         <button id='b' data-state='off' style='position:absolute;left:20px;top:30px;width:80px;height:24px'>Go</button>\
         <div id='hidden' style='display:none'>x</div>\
         <p class='item'>a</p><p class='item'>b</p>\
         <input id='in'>\
         <script>document.getElementById('b').addEventListener('click',function(){this.dataset.state='on';this.textContent='Clicked'});</script>",
    )
    .expect("navigate");

    // Presence / count.
    page.locator(".item").expect().to_have_count(2).expect("two items");
    // Geometry (CDP content box — inside the button's padding/border at left:20/top:30).
    let rect = page.locator("#b").bounding_box().expect("box");
    assert!(
        rect.x >= 20.0 && rect.y >= 30.0 && rect.width > 0.0 && rect.height > 0.0,
        "content box inside the positioned button: {rect:?}"
    );
    // Computed style + visibility.
    page.locator("#b").expect().to_be_visible().expect("button visible");
    page.locator("#hidden").expect().to_be_hidden().expect("hidden div hidden");
    // Attribute.
    page.locator("#b").expect().to_have_attribute("data-state", "off").expect("state off");
    // Trusted click → JS handler mutates state + text.
    page.locator("#b").click().expect("click");
    page.locator("#b").expect().to_have_attribute("data-state", "on").expect("state on after click");
    page.locator("#b").expect().to_have_text("Clicked").expect("text changed");
    // Typed input.
    page.locator("#in").fill("hello").expect("fill");
    let val = page.eval("document.getElementById('in').value").expect("read value");
    assert_eq!(val.as_str(), Some("hello"), "input received typed text");
}
