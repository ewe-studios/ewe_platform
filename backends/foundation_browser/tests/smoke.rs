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
// the browser DOM update. Headful when PRIMAL_TEST_HEADFUL is set — watch it!
//
// Both DomOp wire encodings are exercised against real Chromium: the COLUMNAR
// binary default and the JSON encoder. The page is identical for both — the
// mount-stream's `protocol="arrow"` just means "base64 envelope frames"; the
// runtime's `decodeEnvelopeFrame` dispatches on the envelope's protocol byte
// (1 = columnar → applicator, 2 = json DomOp batch → SAME applicator), so a
// mount renders the same whichever wire it streamed over. Only the App's
// encoder differs. This proves it in an ACTUAL HTTP-server context, not a stub.

/// The shared Mode-1 page: a single targetless `<mount-stream>` + the runtime.
/// No `target` (the App carries absolute primal-ids and `App::mount`s its own
/// root onto `<body>`), no `#app` container — just the mount.
const MODE1_PAGE: &str = r##"<!doctype html><html><head><meta charset=utf-8></head><body>
<h2>spec-43 Mode 1 — native App → real browser</h2>
<mount-stream api="/__primal/stream" transport="sse" protocol="arrow"></mount-stream>
<script type="module">
  import { registerWebComponents } from '/__primal/foundation-wasm-ui.js';
  registerWebComponents();
</script>
</body></html>"##;

/// Mount a greeting component via `app`, then flip a signal — asserting both the
/// initial mount and the live update land in the real browser DOM.
fn drive_greeting(
    app: foundation_wasm_ui::App,
    page: &foundation_browser::Page,
    headful: bool,
) -> foundation_browser::Result<()> {
    use foundation_wasm_ui::html;
    let (ctx, rcv) = app.context();
    let (title, set_title) = ctx.signal(alloc_str("Hello from Rust 👋"));

    let t = title.clone();
    // Build the tree, then `App::mount` splices it onto <body> — the same
    // first-class machinery `App::theme` uses for <head>. No bespoke mount ops.
    app.mount(html! { ctx, rcv,
        <div id="greeting" class="greeting">{t.get()}</div>
    });
    app.stabilize(); // initial mount → SSE → browser applies it
    page.locator("#greeting").expect().to_have_text("Hello from Rust 👋")?;
    if headful {
        std::thread::sleep(std::time::Duration::from_millis(1500));
    }

    set_title.set(alloc_str("Updated live from a Rust signal! ✨"));
    app.stabilize(); // the SetText op streams → DOM updates live
    page.locator("#greeting").expect().to_have_text("Updated live from a Rust signal! ✨")?;
    if headful {
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    Ok(())
}

#[test]
fn mode1_columnar_binary_streams_to_real_browser() {
    use foundation_browser::test::Encoding;
    use foundation_ui_traits::ColumnarEncoder;
    use foundation_wasm_ui::App;

    let headful = std::env::var("PRIMAL_TEST_HEADFUL").is_ok();
    let harness = Harness::setup(TestConfig {
        html: MODE1_PAGE.into(),
        encoding: Encoding::Columnar,
        headless: !headful,
        ..TestConfig::default()
    })
    .expect("setup");

    harness.run("mode1_columnar_binary_streams_to_real_browser", |server, page| {
        // Columnar binary (protocol byte 1) — the default wire.
        let sink = foundation_browser::test::BroadcastSink::with_encoder(
            ColumnarEncoder,
            server.broadcaster(),
        );
        drive_greeting(App::with_protocol(sink), page, headful)
    });
}

#[test]
fn mode1_json_streams_to_real_browser() {
    use foundation_browser::test::Encoding;
    use foundation_ui_traits::JsonEncoder;
    use foundation_wasm_ui::App;

    let headful = std::env::var("PRIMAL_TEST_HEADFUL").is_ok();
    let harness = Harness::setup(TestConfig {
        html: MODE1_PAGE.into(),
        encoding: Encoding::Json,
        headless: !headful,
        ..TestConfig::default()
    })
    .expect("setup");

    harness.run("mode1_json_streams_to_real_browser", |server, page| {
        // JSON DomOp batch (protocol byte 2) — decoded through the SAME envelope
        // + applicator path as columnar. The fix that makes this render: the
        // runtime now routes a JSON DomOp batch to the DomOp applicator.
        let sink = foundation_browser::test::BroadcastSink::with_encoder(
            JsonEncoder,
            server.broadcaster(),
        );
        drive_greeting(App::with_protocol(sink), page, headful)
    });
}

fn alloc_str(s: &str) -> String {
    s.to_string()
}
