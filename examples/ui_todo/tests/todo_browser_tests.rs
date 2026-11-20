//! Test the server and browser-local actions in the same browser page.
use foundation_browser::{
    test::{BacklogConfig, LiveChannel},
    BrowserDriver, LaunchConfig,
};
use foundation_context::Ctx;
use foundation_http::ShutdownSignal;
use foundation_ui_wire::writer::WriterConfig;
use std::{net::TcpListener, path::Path, sync::Arc};

#[test]
fn server_and_host_actions_work() {
    let module = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("module/target/wasm32-unknown-unknown/uat/ui_todo_web.wasm");
    assert!(
        module.exists(),
        "Build examples/ui_todo/build-module.sh first"
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let shutdown = Arc::new(ShutdownSignal::new());
    let stop = shutdown.clone();
    let channel = LiveChannel::new(1, BacklogConfig::default(), WriterConfig::default());
    let served = channel.clone();
    let worker = std::thread::spawn(move || {
        ui_todo::server::serve_with_channel(&listener, &stop, &module, served)
    });
    let outcome = std::panic::catch_unwind(|| {
        let driver = BrowserDriver::launch(LaunchConfig::default().ctx(Ctx::host())).unwrap();
        let other = driver.new_page().unwrap();
        other.goto(&url).unwrap();
        let shared_count = other.locator("#count-value");
        shared_count.expect().to_have_text("0").unwrap();
        let page = driver.new_page().unwrap();
        page.goto(&url).unwrap();
        let button = |name: &str| format!(r#"button[primal\:onclick="{name}"]"#);
        let add = page.locator(&button("host:open_add_dialog"));
        add.expect()
            .to_be_visible()
            .unwrap_or_else(|e| panic!("{e:?}; {:?}", page.eval("document.body.innerText")));
        add.click().unwrap();
        page.locator("input[name=title]")
            .expect()
            .to_be_visible()
            .unwrap_or_else(|e| panic!("{e:?}; {:?}", page.eval("document.body.innerText")));
        page.locator("input[name=title]").fill("buy milk").unwrap();
        page.locator("button[type=submit]").click().unwrap();
        page.locator("#open-count")
            .expect()
            .to_have_text("1")
            .unwrap_or_else(|e| panic!("{e:?}; {:?}", page.eval("document.body.innerText")));
        let toggle = page.locator(&button("server:toggle_todo"));
        toggle.expect().to_have_text("Complete").unwrap();
        shared_count.expect().to_have_text("1").unwrap();
        page.locator("button[value=done]").click().unwrap();
        toggle.expect().to_have_count(0).unwrap();
        page.locator("button[value=all]").click().unwrap();
        toggle.expect().to_be_visible().unwrap();
        other
            .eval(
                r"{
            const fetch = window.fetch.bind(window);
            window.fetch = (url, options) => {
                if (String(url).includes('/live')) window.replayUrl = String(url);
                return fetch(url, options);
            };
        }",
            )
            .unwrap();
        channel.close();
        toggle.click().unwrap();
        shared_count.expect().to_have_text("0").unwrap();
        assert_eq!(
            other
                .eval("new URL(window.replayUrl, location.href).searchParams.get('resume_from')")
                .unwrap()
                .as_str(),
            Some("1")
        );
        page.locator("#open-count")
            .expect()
            .to_have_text("0")
            .unwrap();
        page.locator(&button("host:open_delete_dialog"))
            .click()
            .unwrap();
        let confirm = page.locator(&button("server:confirm_delete"));
        confirm.expect().to_be_visible().unwrap();
        page.eval(r"window.staleTodoButton = Array.from(document.querySelectorAll('button')).find(el => el.getAttribute('primal:onclick') === 'server:confirm_delete').cloneNode(true); true").unwrap();
        confirm.click().unwrap();
        toggle.expect().to_have_count(0).unwrap();
        page.eval(r"{
            document.body.appendChild(window.staleTodoButton);
            window.FoundationWasmUiRuntime.Patcher.runtime.dispatcher.scanAndWire(window.staleTodoButton);
            window.staleTodoButton.click();
        }").unwrap();
        page.locator("#notice")
            .expect()
            .to_have_text("ActionFailed")
            .unwrap();
    });
    shutdown.turn_on();
    worker.join().unwrap().unwrap();
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic)
    }
}
