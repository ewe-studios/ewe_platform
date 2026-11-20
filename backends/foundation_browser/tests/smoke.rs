//! Live CDP smoke test against a real Chromium (spec-43 phase-1 milestone 2).
//!
//! Gated behind `--features browser-tests` so CI without a browser skips it.
//! Validates the whole premise: our pure-Rust WebSocket client speaks CDP to a
//! real browser — launch, navigate, evaluate, screenshot.

#![cfg(feature = "browser-tests")]

use foundation_browser::test::{Harness, TestConfig};
use foundation_browser::{BrowserDriver, LaunchConfig};
use foundation_context::Ctx;

#[test]
fn launch_navigate_eval_screenshot() {
    let driver =
        BrowserDriver::launch(LaunchConfig::chromium().ctx(Ctx::host())).expect("launch chromium");
    let page = driver.new_page().expect("attach a page");

    page.goto("data:text/html,<h1 id='hi'>hello browser</h1>")
        .expect("navigate");

    let text = page
        .eval("document.getElementById('hi').textContent")
        .expect("evaluate");
    assert_eq!(text.as_str(), Some("hello browser"), "DOM read over CDP");

    let out = std::path::Path::new("target/primal-test/smoke.png");
    page.screenshot(out).expect("screenshot");
    let meta = std::fs::metadata(out).expect("screenshot file written");
    assert!(
        meta.len() > 100,
        "screenshot should be a non-trivial PNG ({} bytes)",
        meta.len()
    );
}

#[test]
fn locator_geometry_attributes_style_and_input() {
    let driver =
        BrowserDriver::launch(LaunchConfig::chromium().ctx(Ctx::host())).expect("launch chromium");
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
    page.locator(".item")
        .expect()
        .to_have_count(2)
        .expect("two items");
    // Geometry (CDP content box — inside the button's padding/border at left:20/top:30).
    let rect = page.locator("#b").bounding_box().expect("box");
    assert!(
        rect.x >= 20.0 && rect.y >= 30.0 && rect.width > 0.0 && rect.height > 0.0,
        "content box inside the positioned button: {rect:?}"
    );
    // Computed style + visibility.
    page.locator("#b")
        .expect()
        .to_be_visible()
        .expect("button visible");
    page.locator("#hidden")
        .expect()
        .to_be_hidden()
        .expect("hidden div hidden");
    // Attribute.
    page.locator("#b")
        .expect()
        .to_have_attribute("data-state", "off")
        .expect("state off");
    // Trusted click → JS handler mutates state + text.
    page.locator("#b").click().expect("click");
    page.locator("#b")
        .expect()
        .to_have_attribute("data-state", "on")
        .expect("state on after click");
    page.locator("#b")
        .expect()
        .to_have_text("Clicked")
        .expect("text changed");
    // Typed input.
    page.locator("#in").fill("hello").expect("fill");
    let val = page
        .eval("document.getElementById('in').value")
        .expect("read value");
    assert_eq!(val.as_str(), Some("hello"), "input received typed text");
}

#[test]
fn harness_serves_drives_and_tears_down() {
    let harness = Harness::setup(TestConfig {
        html: "<button id='go'>Go</button><div id='out'></div>\
               <script>document.getElementById('go').onclick=function(){document.getElementById('out').textContent='done'}</script>"
            .into(),
        ..TestConfig::default()
    })
    .expect("setup server + browser");

    harness.run("harness_serves_drives_and_tears_down", |_server, page| {
        page.locator("#go").click()?;
        page.locator("#out").expect().to_have_text("done")?;
        Ok(())
    });
    // run() consumed the harness → server thread + browser process torn down here.
}

// The ergonomic form: the macro owns setup + teardown.
#[foundation_browser::wasm_ui_server(
    html = "<button id='go'>Go</button><div id='out'></div><script>document.getElementById('go').onclick=function(){document.getElementById('out').textContent='ok'}</script>"
)]
fn macro_drives_a_served_page(
    _server: &foundation_browser::test::TestServer,
    page: &foundation_browser::Page,
) -> foundation_browser::Result<()> {
    page.locator("#go").click()?;
    page.locator("#out").expect().to_have_text("ok")?;
    Ok(())
}

#[test]
fn file_page_source_and_static_directory() {
    use std::io::Write;

    // A temp site: an HTML page file + an asset under assets/.
    let dir = std::env::temp_dir().join(format!("primal-site-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::File::create(dir.join("assets/hello.txt"))
        .unwrap()
        .write_all(b"asset-ok")
        .unwrap();
    let page_file = dir.join("page.html");
    std::fs::File::create(&page_file)
        .unwrap()
        .write_all(
            b"<div id='m'>file-page</div>\
              <script>fetch('/assets/hello.txt').then(function(r){return r.text()}).then(function(t){\
                var d=document.createElement('div');d.id='asset';d.textContent=t;document.body.appendChild(d)})</script>",
        )
        .unwrap();

    let harness = Harness::setup(TestConfig {
        file: Some(page_file),
        static_dir: Some(dir.clone()),
        static_mount: "/assets".into(),
        ..TestConfig::default()
    })
    .expect("setup");

    harness.run("file_page_source_and_static_directory", |_server, page| {
        page.locator("#m").expect().to_have_text("file-page")?; // File page source served
        page.locator("#asset").expect().to_have_text("asset-ok")?; // StaticFileHandler served the dir
        Ok(())
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn channel_a_streams_a_frame_to_the_browser() {
    // The page opens the channel-A SSE stream and renders each frame (base64 → text).
    let harness = Harness::setup(TestConfig {
        html: "<div id='out'></div><script>\
               var es=new EventSource('/__primal/stream');\
               es.onmessage=function(e){var d=document.createElement('div');d.className='frame';\
                 d.textContent=atob(e.data);document.getElementById('out').appendChild(d)};\
               </script>"
            .into(),
        ..TestConfig::default()
    })
    .expect("setup");

    harness.run(
        "channel_a_streams_a_frame_to_the_browser",
        |server, page| {
            // Push a frame from Rust → broadcaster → SSE → browser. Backlog/replay
            // absorbs the connect/push race; the assertion retries.
            server.push_frame(b"hello-stream");
            page.locator(".frame")
                .expect()
                .to_have_text("hello-stream")?;
            Ok(())
        },
    );
}

/// The messages the live browser cycles through (mounted, then live updates).
const MESSAGES: [&str; 4] = [
    "Hello from Rust 👋",
    "Updated live from a Rust signal! ✨",
    "foundation_wasm_ui rendering from tests in live browsers",
    "foundation_wasm_ui saying good bye",
];

/// 800×800 so a headful window is comfortably visible (no effect headless).
const WINDOW: Option<(u32, u32)> = Some((800, 800));

fn is_headful() -> bool {
    std::env::var("PRIMAL_TEST_HEADFUL").is_ok()
}

fn watch_pause(headful: bool) {
    if headful {
        std::thread::sleep(std::time::Duration::from_millis(1400));
    }
}

fn protocol_page(title: &str, frame: bool, body: &str) -> String {
    let (api, protocol) = if frame {
        ("/__primal/live", "frame")
    } else {
        ("/__primal/stream", "html")
    };
    format!(
        r#"<!doctype html><meta charset=utf-8><title>{title}</title><h1>{title}</h1>{body}
<mount-stream api="{api}" transport="sse" protocol="{protocol}"></mount-stream>
<script type="module">
  import * as ui from '/__primal/foundation-wasm-ui.js';
  import {{installFrameRuntime}} from '/__primal/foundation-ui-frames.js';
  installFrameRuntime(ui); ui.registerWebComponents();
</script>"#
    )
}

/// Mount the greeting with EWF1 and update its signal
/// through every message, asserting each lands in the real browser.
fn drive_greeting(
    app: foundation_wasm_ui::App,
    page: &foundation_browser::Page,
    headful: bool,
) -> foundation_browser::Result<()> {
    use foundation_wasm_ui::html;
    let (ctx, rcv) = app.context();
    let (title, set_title) = ctx.signal(alloc_str(MESSAGES[0]));

    let t = title.clone();
    // Build the tree, then `App::mount` splices it onto <body> — the same
    // first-class machinery `App::theme` uses for <head>. No bespoke mount ops.
    app.mount(html! { ctx, rcv,
        <div id="greeting" class="greeting">{t.get()}</div>
    });
    app.stabilize().unwrap(); // initial mount → SSE → browser applies it
    page.locator("#greeting")
        .expect()
        .to_have_text(MESSAGES[0])?;
    watch_pause(headful);

    for msg in &MESSAGES[1..] {
        set_title.set(alloc_str(msg)); // drive a SIGNAL in Rust
        app.stabilize().unwrap(); // the SetText op streams → DOM updates live
        page.locator("#greeting").expect().to_have_text(msg)?;
        watch_pause(headful);
    }
    Ok(())
}

#[test]
fn frame_updates_a_real_browser() {
    let headful = is_headful();
    let harness = Harness::setup(TestConfig {
        html: protocol_page("Live frames", true, ""),
        headless: !headful,
        window_size: WINDOW,
        ..TestConfig::default()
    })
    .unwrap();
    harness.run("frame_updates_a_real_browser", |server, page| {
        drive_greeting(
            foundation_wasm_ui::App::with_protocol(server.broadcast_sink()),
            page,
            headful,
        )
    });
}

#[test]
fn mode1_html_streams_to_real_browser() {
    let headful = is_headful();
    // The server can deliver HTML directly: the server streams markup, the runtime's
    // routeHtml morphs it into a stable `#stage`. No App / encoder here — the
    // broadcaster's text frame is the whole story.
    let page = protocol_page("html (markup)", false, "<div id=\"stage\"></div>");
    let harness = Harness::setup(TestConfig {
        html: page,
        headless: !headful,
        window_size: WINDOW,
        ..TestConfig::default()
    })
    .expect("setup");

    harness.run("mode1_html", |server, page| {
        for msg in MESSAGES {
            // An island fragment morphs #stage's children → the greeting.
            server.push_html(&format!(
                "<island data-target=\"#stage\" data-action=\"replace-children\">\
                 <div id=\"greeting\" class=\"greeting\">{msg}</div></island>"
            ));
            page.locator("#greeting").expect().to_have_text(msg)?;
            watch_pause(headful);
        }
        Ok(())
    });
}

// HTTPS — the TestServer serves TLS with the bundled localhost dev cert; the
// browser trusts it (--ignore-certificate-errors). Proves secure-context pages
// load and a secure-context-only API (crypto.subtle) is available.
#[test]
fn https_serves_a_secure_context() {
    let harness = Harness::setup(TestConfig {
        html: "<h1 id='h'>secure</h1>\
               <script>window.__secure = window.isSecureContext && !!(crypto.subtle);</script>"
            .into(),
        https: true,
        ..TestConfig::default()
    })
    .expect("setup https");

    harness.run("https_serves_a_secure_context", |_server, page| {
        page.locator("#h").expect().to_have_text("secure")?;
        let secure = page.eval("window.__secure").unwrap_or_default();
        assert_eq!(
            secure.as_bool(),
            Some(true),
            "page is a secure context over TLS"
        );
        Ok(())
    });
}

// The primal-test helper (spec-43 §6): the runtime's frame instrument +
// `page.wait_for_reactive` + `page.bounding_boxes`, and the served in-page
// `window.__primalTest` API.
#[test]
fn primal_test_helper_and_reactive_settle() {
    use foundation_wasm_ui::{html, App};

    // Page loads the runtime AND the injected helper (proves both are served).
    let page_html = r##"<!doctype html><html><head><meta charset=utf-8></head><body>
<mount-stream api="/__primal/live" transport="sse" protocol="frame"></mount-stream>
<script type="module">
  import * as ui from '/__primal/foundation-wasm-ui.js';
  import {installFrameRuntime} from '/__primal/foundation-ui-frames.js';
  import { installPrimalTest } from '/__primal/primal-test.js';
  installFrameRuntime(ui); ui.registerWebComponents();
  installPrimalTest();
</script>
</body></html>"##;

    let harness = Harness::setup(TestConfig {
        html: page_html.into(),
        ..TestConfig::default()
    })
    .expect("setup");

    harness.run("primal_test_helper_and_reactive_settle", |server, page| {
        let app = App::with_protocol(server.broadcast_sink());
        let (ctx, rcv) = app.context();
        let (label, set_label) = ctx.signal(alloc_str("one"));
        let l = label.clone();
        app.mount(html! { ctx, rcv, <p id="p">{l.get()}</p> });
        app.stabilize().unwrap();

        // Driver-side: wait until the streamed frame has settled, no in-page helper needed.
        assert!(page.wait_for_reactive(40, 4000)?, "DOM settled after the mount frame");
        page.locator("#p").expect().to_have_text("one")?;

        set_label.set(alloc_str("two"));
        app.stabilize().unwrap();
        assert!(page.wait_for_reactive(40, 4000)?, "DOM settled after the update frame");
        page.locator("#p").expect().to_have_text("two")?;

        // Batched layout read of several selectors in one pass.
        let boxes = page.bounding_boxes(&["#p", "#missing"]).unwrap_or_default();
        assert!(boxes.get("#p").map(|b| !b.is_null()).unwrap_or(false), "#p has a box");
        assert!(boxes.get("#missing").map(serde_json::Value::is_null).unwrap_or(false), "#missing is null");

        // In-page helper: the served primal-test.js installed window.__primalTest.
        let has_helper = page
            .eval("typeof window.__primalTest === 'object' && typeof window.__primalTest.waitForReactive === 'function'")
            .unwrap_or_default();
        assert_eq!(has_helper.as_bool(), Some(true), "window.__primalTest installed");
        Ok(())
    });
}

// ── Phase 2: Firefox over WebDriver BiDi (spec-43 §7) ────────────────────────
// The SAME Page/Locator API drives Firefox via BiDi (Firefox blocks top-level
// `data:` navigation, so these go through the served HTTP harness). Skipped
// gracefully when Firefox isn't installed.

fn firefox_or_skip(config: TestConfig) -> Option<Harness> {
    use foundation_browser::{Browser, BrowserError};
    match Harness::setup(TestConfig {
        browser: Browser::Firefox,
        ..config
    }) {
        Ok(h) => Some(h),
        Err(BrowserError::BrowserNotInstalled { .. }) => {
            eprintln!(
                "skipping Firefox test: firefox not installed (mise run test:browsers:firefox)"
            );
            None
        }
        Err(e) => panic!("firefox harness setup: {e}"),
    }
}

// Parity with `locator_geometry_attributes_style_and_input` (Chromium #2),
// served over HTTP (Firefox blocks top-level `data:` navigation). Same Locator
// surface — count, geometry, visibility, attributes, computed style, trusted
// click/hover/fill — over BiDi instead of CDP.
#[test]
fn firefox_locator_geometry_attributes_style_and_input() {
    let Some(harness) = firefox_or_skip(TestConfig {
        html: "<button id='b' data-state='off' style='position:absolute;left:20px;top:30px;width:80px;height:24px'>Go</button>\
               <div id='hidden' style='display:none'>x</div>\
               <p class='item'>a</p><p class='item'>b</p>\
               <input id='in'>\
               <script>document.getElementById('b').addEventListener('click',function(){this.dataset.state='on';this.textContent='Clicked'});</script>"
            .into(),
        headless: !is_headful(),
        window_size: WINDOW,
        ..TestConfig::default()
    }) else {
        return;
    };

    harness.run(
        "firefox_locator_geometry_attributes_style_and_input",
        |_server, page| {
            page.locator(".item").expect().to_have_count(2)?;
            let rect = page.locator("#b").bounding_box()?;
            assert!(
                rect.x >= 20.0 && rect.y >= 30.0 && rect.width > 0.0,
                "box: {rect:?}"
            );
            page.locator("#b").expect().to_be_visible()?;
            page.locator("#hidden").expect().to_be_hidden()?;
            page.locator("#b")
                .expect()
                .to_have_attribute("data-state", "off")?;
            page.locator("#b").click()?;
            page.locator("#b")
                .expect()
                .to_have_attribute("data-state", "on")?;
            page.locator("#b").expect().to_have_text("Clicked")?;
            page.locator("#b").hover()?; // trusted pointer move (no panic = ok)
            page.locator("#in").fill("hello")?;
            let val = page
                .eval("document.getElementById('in').value")
                .unwrap_or_default();
            assert_eq!(val.as_str(), Some("hello"), "typed text reached the field");
            Ok(())
        },
    );
}

#[test]
fn firefox_frame_updates_a_real_browser() {
    let headful = is_headful();
    let Some(harness) = firefox_or_skip(TestConfig {
        html: protocol_page("Firefox live frames", true, ""),
        headless: !headful,
        window_size: WINDOW,
        ..TestConfig::default()
    }) else {
        return;
    };
    harness.run("firefox_frame_updates_a_real_browser", |server, page| {
        drive_greeting(
            foundation_wasm_ui::App::with_protocol(server.broadcast_sink()),
            page,
            headful,
        )
    });
}

#[test]
fn firefox_mode1_html_streams_to_real_browser() {
    let page = protocol_page("firefox · html", false, "<div id=\"stage\"></div>");
    let Some(harness) = firefox_or_skip(TestConfig {
        html: page,
        headless: !is_headful(),
        window_size: WINDOW,
        ..TestConfig::default()
    }) else {
        return;
    };
    let headful = is_headful();
    harness.run("firefox_mode1_html", |server, page| {
        for msg in MESSAGES {
            server.push_html(&format!(
                "<island data-target=\"#stage\" data-action=\"replace-children\">\
                 <div id=\"greeting\" class=\"greeting\">{msg}</div></island>"
            ));
            page.locator("#greeting").expect().to_have_text(msg)?;
            watch_pause(headful);
        }
        Ok(())
    });
}

#[test]
fn firefox_primal_test_helper_and_reactive_settle() {
    use foundation_wasm_ui::{html, App};

    let page_html = r##"<!doctype html><html><head><meta charset=utf-8></head><body>
<mount-stream api="/__primal/live" transport="sse" protocol="frame"></mount-stream>
<script type="module">
  import * as ui from '/__primal/foundation-wasm-ui.js';
  import {installFrameRuntime} from '/__primal/foundation-ui-frames.js';
  import { installPrimalTest } from '/__primal/primal-test.js';
  installFrameRuntime(ui); ui.registerWebComponents();
  installPrimalTest();
</script>
</body></html>"##;
    let Some(harness) = firefox_or_skip(TestConfig {
        html: page_html.into(),
        ..TestConfig::default()
    }) else {
        return;
    };

    harness.run("firefox_primal_test_helper_and_reactive_settle", |server, page| {
        let app = App::with_protocol(server.broadcast_sink());
        let (ctx, rcv) = app.context();
        let (label, set_label) = ctx.signal(alloc_str("one"));
        let l = label.clone();
        app.mount(html! { ctx, rcv, <p id="p">{l.get()}</p> });
        app.stabilize().unwrap();
        assert!(page.wait_for_reactive(40, 4000)?, "DOM settled after mount (BiDi)");
        page.locator("#p").expect().to_have_text("one")?;

        set_label.set(alloc_str("two"));
        app.stabilize().unwrap();
        assert!(page.wait_for_reactive(40, 4000)?, "DOM settled after update (BiDi)");
        page.locator("#p").expect().to_have_text("two")?;

        let has_helper = page
            .eval("typeof window.__primalTest === 'object' && typeof window.__primalTest.waitForReactive === 'function'")
            .unwrap_or_default();
        assert_eq!(has_helper.as_bool(), Some(true), "window.__primalTest installed (Firefox)");
        Ok(())
    });
}

fn alloc_str(s: &str) -> String {
    s.to_string()
}

/// Deliver Rust frame bytes over server-sent events and apply them in Chromium.
#[test]
fn ewf1_stream_updates_the_browser() {
    let html = include_str!("fixtures/ewf1-page.html");
    let harness = Harness::setup(TestConfig {
        html: html.into(),
        ..TestConfig::default()
    })
    .expect("start the test page");
    harness.run("ewf1_stream_updates_the_browser", |server, page| {
        server.broadcaster().send(include_bytes!(
            "../../foundation_ui_wire/fixtures/frame_ops_counter.bin"
        ));
        page.locator("#count").expect().to_have_text("1")?;
        page.locator("#local-result").expect().to_have_text("ok")?;
        Ok(())
    });
}

/// Compare local and streamed delivery of the same Rust frame.
#[test]
fn wasm_and_sse_render_the_same_button() {
    let backends = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let harness = Harness::setup(TestConfig {
        html: include_str!("fixtures/wire-parity.html").into(),
        static_dir: Some(backends.into()),
        static_mount: "/foundation_wasm_ui".into(),
        ..TestConfig::default()
    })
    .unwrap();
    harness.run("wasm_and_sse_render_the_same_button", |server, page| {
        page.locator("#result").expect().to_have_text("ready")?;
        let bytes: Vec<u8> = serde_json::from_value(page.eval("window.parityBytes")?).unwrap();
        assert_eq!(bytes.get(4..8), Some(&b"EWF1"[..]));
        server.broadcaster().send(&bytes);
        page.locator("#result").expect().to_have_text("matched")?;
        Ok(())
    });
}
