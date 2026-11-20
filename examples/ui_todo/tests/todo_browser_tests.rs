//! Test the server and browser-local actions in the same browser page.
use foundation_browser::{BrowserDriver, LaunchConfig};
use foundation_context::Ctx;
use foundation_http::ShutdownSignal;
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
    let worker = std::thread::spawn(move || ui_todo::server::serve(&listener, &stop, &module));
    let outcome = std::panic::catch_unwind(|| {
        let driver = BrowserDriver::launch(LaunchConfig::default().ctx(Ctx::host())).unwrap();
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
        page.locator("button[value=done]").click().unwrap();
        toggle.expect().to_have_count(0).unwrap();
        page.locator("button[value=all]").click().unwrap();
        toggle.expect().to_be_visible().unwrap();
        toggle.click().unwrap();
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
