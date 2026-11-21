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
        let app = ui_todo::server::app_with_channel(&module, served)?;
        app.server(&listener.local_addr()?.to_string())
            .serve_with_listener(&listener, &stop);
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
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
        check_container_layout(&page);
        check_cancel_and_submit(&page, &add);
        page.locator("#open-count")
            .expect()
            .to_have_text("1")
            .unwrap_or_else(|e| panic!("{e:?}; {:?}", page.eval("document.body.innerText")));
        let toggle = page.locator(&button("server:toggle_todo"));
        toggle.expect().to_have_text("Complete").unwrap();
        shared_count.expect().to_have_text("1").unwrap();
        submit_with_keyboard(&page, &add, "second todo");
        shared_count.expect().to_have_text("2").unwrap();
        let second_delete =
            page.locator(r#"button[primal\:onclick="host:open_delete_dialog"][value="2"]"#);
        second_delete.expect().to_be_visible().unwrap();
        second_delete.click().unwrap();
        let confirm_second = page.locator(&button("server:confirm_delete"));
        confirm_second.expect().to_be_visible().unwrap();
        confirm_second.click().unwrap();
        page.locator(&button("server:confirm_delete"))
            .expect()
            .to_have_count(0)
            .unwrap();
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
            Some("3")
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

fn submit_with_keyboard(
    page: &foundation_browser::Page<'_>,
    add: &foundation_browser::Locator<'_>,
    title: &str,
) {
    add.click().unwrap();
    let input = page.locator("dialog:modal input");
    input.expect().to_be_visible().unwrap();
    input.fill(title).unwrap();
    input.press("Enter").unwrap();
}

fn check_cancel_and_submit(
    page: &foundation_browser::Page<'_>,
    add: &foundation_browser::Locator<'_>,
) {
    add.click().unwrap();
    page.locator("dialog:modal input")
        .expect()
        .to_be_visible()
        .unwrap_or_else(|e| panic!("{e:?}; {:?}", page.eval("document.body.innerText")));
    let input = page.locator("dialog:modal input");
    page.locator("dialog:modal")
        .expect()
        .to_have_count(1)
        .unwrap();
    assert_eq!(
        page.eval("document.activeElement === document.querySelector('dialog:modal input')")
            .unwrap(),
        true
    );
    input.press("Escape").unwrap();
    page.locator("dialog:modal")
        .expect()
        .to_have_count(0)
        .unwrap();
    add.click().unwrap();
    input.expect().to_be_visible().unwrap();
    input.fill("buy milk").unwrap();
    page.locator("dialog:modal [data-ewe-presentation=submit-action]")
        .click()
        .unwrap();
}

/// Apply the same frame as a mobile host and inspect the browser layout.
fn check_container_layout(page: &foundation_browser::Page<'_>) {
    use foundation_ui_runtime::{
        html,
        view::{lower, LowerOptions, TokenTable},
    };
    use foundation_ui_wire::{
        ops,
        writer::{write_messages, ClientCaps, FrameHeader},
        UiOperationT,
    };
    let tree = html! { <Stack height="160">
        <Text>Behind the overlay</Text>
        <BottomSheet><Text>Inline panel</Text></BottomSheet>
        <Overlay><BottomSheet><Text>Floating panel</Text></BottomSheet></Overlay>
    </Stack> };
    let mut operations = lower(
        &tree,
        &LowerOptions {
            id_base: 1_000_000,
            tokens: &TokenTable::new(),
        },
    )
    .unwrap();
    let UiOperationT::CreateNode(root) = &operations[0].ui else {
        panic!("A stack must create a root node");
    };
    let root = root.node_id;
    operations.push(ops::append_child(1, root));
    let send = |ops: Vec<foundation_ui_wire::OperationT>| {
        let (bytes, _) = write_messages(
            ops.into_iter().map(ops::message).collect(),
            &ClientCaps::web_current(),
            FrameHeader {
                stream_id: 0,
                sequence_id: 0,
                ack: 0,
            },
            &WriterConfig::default(),
        )
        .unwrap();
        page.eval(&format!("window.FoundationWasmUiRuntime.Patcher.runtime.frames.receive(Uint8Array.from({bytes:?}))")).unwrap();
    };
    send(operations);
    page.locator(".ewe-overlay > .ewe-bottom-sheet")
        .expect()
        .to_be_visible()
        .unwrap();
    let matches = page
        .eval(
            r#"(() => {
        const floating = document.querySelector('.ewe-overlay > .ewe-bottom-sheet');
        const root = floating.parentElement.parentElement;
        const rect = node => node.getBoundingClientRect();
        const near = (a, b) => Math.abs(a - b) <= 1;
        const inline = root.querySelector(':scope > .ewe-bottom-sheet');
        const button = [...document.querySelectorAll("button")].find(node => node.getAttribute("primal:onclick") === "host:open_add_dialog");
        const {x, y, width, height} = rect(button);
        const hit = document.elementFromPoint(x + width / 2, y + height / 2);
        return near(rect(floating).bottom, innerHeight)
            && near(rect(inline).bottom, rect(root).bottom)
            && rect(root).height === 160 && button.contains(hit);
    })()"#,
        )
        .unwrap();
    assert_eq!(matches.as_bool(), Some(true));
    send(vec![ops::remove_node(root)]);
    page.locator(".ewe-overlay")
        .expect()
        .to_have_count(0)
        .unwrap();
}
