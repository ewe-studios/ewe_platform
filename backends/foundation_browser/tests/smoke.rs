//! Live CDP smoke test against a real Chromium (spec-43 phase-1 milestone 2).
//!
//! Gated behind `--features browser-tests` so CI without a browser skips it.
//! Validates the whole premise: our pure-Rust WebSocket client speaks CDP to a
//! real browser — launch, navigate, evaluate, screenshot.

#![cfg(feature = "browser-tests")]

use foundation_browser::test::{Harness, TestConfig};
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
#[foundation_browser::wasm_ui_server(html = "<button id='go'>Go</button><div id='out'></div><script>document.getElementById('go').onclick=function(){document.getElementById('out').textContent='ok'}</script>")]
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

    harness.run("channel_a_streams_a_frame_to_the_browser", |server, page| {
        // Push a frame from Rust → broadcaster → SSE → browser. Backlog/replay
        // absorbs the connect/push race; the assertion retries.
        server.push_frame(b"hello-stream");
        page.locator(".frame").expect().to_have_text("hello-stream")?;
        Ok(())
    });
}

// Mode 1 — a NATIVE foundation_wasm_ui App streams to a REAL browser. The App
// runs in this test thread; its protocol sink ships frames over channel-A SSE;
// the page's <mount-stream> applies them. We drive a signal in Rust and watch
// the browser DOM cycle through the MESSAGES. Headful + an 800×800 window when
// PRIMAL_TEST_HEADFUL is set — watch it!
//
// All FOUR protocols render the SAME component in real Chromium over the real
// foundation_http server, each with its own page <title>/<h1>:
//   * columnar  — our compact binary wire (protocol 1 v1)
//   * arrow     — Apache Arrow IPC (protocol 1 v2), via the bundled apache-arrow
//   * json      — JSON DomOp batch (protocol 2)
//   * html      — raw markup (island morph), the non-DomOp delivery
// The three DomOp wires share ONE applicator (the envelope's protocol byte is
// the only difference); html rides `routeHtml`. Nothing bespoke in the test —
// every capability is foundation_wasm_ui / foundation_http machinery.

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

/// A Mode-1 page: a `<title>`/`<h1>` naming the protocol, plus the `<mount-stream>`
/// and runtime. `mount_attrs` selects the wire path; `extra_head`/`extra_module`
/// carry per-protocol setup (e.g. apache-arrow + `registerArrowIpc()`).
fn protocol_page(
    title: &str,
    mount_attrs: &str,
    body_extra: &str,
    extra_head: &str,
    extra_module: &str,
) -> String {
    format!(
        "<!doctype html><html><head><meta charset=utf-8><title>{title}</title>{extra_head}</head>\
<body>\n<h1>{title}</h1>\n{body_extra}\n\
<mount-stream api=\"/__primal/stream\" transport=\"sse\" {mount_attrs}></mount-stream>\n\
<script type=\"module\">\n\
  import {{ registerWebComponents, registerArrowIpc }} from '/__primal/foundation-wasm-ui.js';\n\
  registerWebComponents();\n  {extra_module}\n\
</script>\n</body></html>"
    )
}

/// Drive the greeting component over a DomOp wire: mount, then flip the signal
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
    app.stabilize(); // initial mount → SSE → browser applies it
    page.locator("#greeting").expect().to_have_text(MESSAGES[0])?;
    watch_pause(headful);

    for msg in &MESSAGES[1..] {
        set_title.set(alloc_str(msg)); // drive a SIGNAL in Rust
        app.stabilize(); // the SetText op streams → DOM updates live
        page.locator("#greeting").expect().to_have_text(msg)?;
        watch_pause(headful);
    }
    Ok(())
}

#[test]
fn mode1_columnar_binary_streams_to_real_browser() {
    use foundation_browser::test::{BroadcastSink, Encoding};
    use foundation_ui_traits::ColumnarEncoder;
    use foundation_wasm_ui::App;

    let headful = is_headful();
    let page = protocol_page("columnar (custom binary)", "protocol=\"arrow\"", "", "", "");
    let harness = Harness::setup(TestConfig {
        html: page,
        encoding: Encoding::Columnar,
        headless: !headful,
        window_size: WINDOW,
        ..TestConfig::default()
    })
    .expect("setup");

    harness.run("mode1_columnar", |server, page| {
        let sink = BroadcastSink::with_encoder(ColumnarEncoder, server.broadcaster());
        drive_greeting(App::with_protocol(sink), page, headful)
    });
}

#[test]
fn mode1_arrow_ipc_streams_to_real_browser() {
    use foundation_browser::test::{BroadcastSink, Encoding};
    use foundation_wasm_ui::{App, ArrowIpcEncoder};

    let headful = is_headful();
    // Arrow IPC needs the apache-arrow reader — loaded as a classic script (sets
    // globalThis.Arrow) then wired by the framework's `registerArrowIpc()`.
    let page = protocol_page(
        "arrow (Apache Arrow IPC)",
        "protocol=\"arrow\"",
        "",
        "<script src=\"/__primal/apache-arrow.js\"></script>",
        "registerArrowIpc();",
    );
    let harness = Harness::setup(TestConfig {
        html: page,
        encoding: Encoding::Arrow,
        headless: !headful,
        window_size: WINDOW,
        ..TestConfig::default()
    })
    .expect("setup");

    harness.run("mode1_arrow_ipc", |server, page| {
        let sink = BroadcastSink::with_encoder(ArrowIpcEncoder, server.broadcaster());
        drive_greeting(App::with_protocol(sink), page, headful)
    });
}

#[test]
fn mode1_json_streams_to_real_browser() {
    use foundation_browser::test::{BroadcastSink, Encoding};
    use foundation_ui_traits::JsonEncoder;
    use foundation_wasm_ui::App;

    let headful = is_headful();
    let page = protocol_page("json", "protocol=\"arrow\"", "", "", "");
    let harness = Harness::setup(TestConfig {
        html: page,
        encoding: Encoding::Json,
        headless: !headful,
        window_size: WINDOW,
        ..TestConfig::default()
    })
    .expect("setup");

    harness.run("mode1_json", |server, page| {
        let sink = BroadcastSink::with_encoder(JsonEncoder, server.broadcaster());
        drive_greeting(App::with_protocol(sink), page, headful)
    });
}

#[test]
fn mode1_html_streams_to_real_browser() {
    let headful = is_headful();
    // HTML is the non-DomOp delivery: the server streams markup, the runtime's
    // routeHtml morphs it into a stable `#stage`. No App / encoder here — the
    // broadcaster's text frame is the whole story.
    let page = protocol_page("html (markup)", "", "<div id=\"stage\"></div>", "", "");
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
        assert_eq!(secure.as_bool(), Some(true), "page is a secure context over TLS");
        Ok(())
    });
}

// The primal-test helper (spec-43 §6): the runtime's frame instrument +
// `page.wait_for_reactive` + `page.bounding_boxes`, and the served in-page
// `window.__primalTest` API.
#[test]
fn primal_test_helper_and_reactive_settle() {
    use foundation_browser::test::{BroadcastSink, Encoding};
    use foundation_ui_traits::ColumnarEncoder;
    use foundation_wasm_ui::{html, App};

    // Page loads the runtime AND the injected helper (proves both are served).
    let page_html = r##"<!doctype html><html><head><meta charset=utf-8></head><body>
<mount-stream api="/__primal/stream" transport="sse" protocol="arrow"></mount-stream>
<script type="module">
  import { registerWebComponents } from '/__primal/foundation-wasm-ui.js';
  import { installPrimalTest } from '/__primal/primal-test.js';
  registerWebComponents();
  installPrimalTest();
</script>
</body></html>"##;

    let harness = Harness::setup(TestConfig {
        html: page_html.into(),
        encoding: Encoding::Columnar,
        ..TestConfig::default()
    })
    .expect("setup");

    harness.run("primal_test_helper_and_reactive_settle", |server, page| {
        let app = App::with_protocol(BroadcastSink::with_encoder(ColumnarEncoder, server.broadcaster()));
        let (ctx, rcv) = app.context();
        let (label, set_label) = ctx.signal(alloc_str("one"));
        let l = label.clone();
        app.mount(html! { ctx, rcv, <p id="p">{l.get()}</p> });
        app.stabilize();

        // Driver-side: wait until the streamed frame has settled, no in-page helper needed.
        assert!(page.wait_for_reactive(40, 4000)?, "DOM settled after the mount frame");
        page.locator("#p").expect().to_have_text("one")?;

        set_label.set(alloc_str("two"));
        app.stabilize();
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

fn alloc_str(s: &str) -> String {
    s.to_string()
}
