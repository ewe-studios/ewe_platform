//! Test the server and browser-local actions in the same browser page.
#[path = "support/browser_server.rs"]
mod browser_server;
use browser_server::{open_document, with_browser_server};
use foundation_browser::{BrowserDriver, LaunchConfig};
use foundation_context::Ctx;
use foundation_ui_wire::writer::WriterConfig;

/// WHY: The packaged Todo page must run its web action without using the server.
/// WHAT: Activate the focused title and verify that its page script scrolls the viewport.
#[test]
fn packaged_todo_header_runs_a_local_page_script() {
    with_browser_server(|driver, url, _| {
        let page = driver.new_page().unwrap();
        open_document(&page, url);
        let header = page.locator("#todo-list-heading");
        header.expect().to_be_visible().unwrap();
        assert_eq!(
            page.eval(
                r##"(async () => {
                while (!window.todoReady) await new Promise(requestAnimationFrame);
                document.querySelector("#page").style.minHeight = "200vh";
                document.querySelector("#todo-list-heading").focus();
                const send = window.fetch.bind(window);
                window.scrollActionRequests = 0;
                window.fetch = (...args) => {
                    window.scrollActionRequests++;
                    return send(...args);
                };
                window.scrollTo(0, 300);
                return window.scrollY > 0;
            })()"##
            )
            .unwrap()
            .as_bool(),
            Some(true)
        );
        header.press(" ").unwrap();
        assert_eq!(
            page.eval(
                r##"(async () => {
                await new Promise(requestAnimationFrame);
                const header = document.querySelector("#todo-list-heading");
                return [window.scrollY, window.scrollActionRequests, document.activeElement.id,
                    document.querySelector("#notice").value, header.getAttribute("primal:onclick")];
            })()"##
            )
            .unwrap(),
            serde_json::json!([0, 0, "todo-list-heading", "", "host:scroll_to_top"])
        );

        let done = r#"#todo-filter button[value="done"]"#;
        page.locator(done).click().unwrap();
        page.locator(&format!(r#"{done}[aria-pressed="true"]"#))
            .expect()
            .to_be_visible()
            .unwrap();
        assert_eq!(
            page.eval("window.scrollActionRequests").unwrap().as_u64(),
            Some(0)
        );
    });
}

#[test]
fn server_and_host_actions_work() {
    with_browser_server(|driver, url, channel| {
        let other = driver.new_page().unwrap();
        open_document(&other, url);
        let shared_count = other.locator("#open-count");
        let peer_toggle =
            other.locator(r#"button[primal\:onclick="server:toggle_todo"]:not([hidden] *)"#);
        shared_count.expect().to_have_text("0 open todos").unwrap();
        assert_eq!(
            other.eval(r#"performance.getEntriesByType("resource").map(entry => new URL(entry.name).pathname).filter(path => path.endsWith(".js")).join(",")"#).unwrap().as_str(),
            Some("/pkg/bundle.js")
        );
        let page = driver.new_page().unwrap();
        open_document(&page, url);
        let button = |name: &str| format!(r#"button[primal\:onclick="{name}"]:not([hidden] *)"#);
        let add = page.locator(&button("host:open_add_dialog"));
        add.expect()
            .to_be_visible()
            .unwrap_or_else(|e| panic!("{e:?}; {:?}", page.eval("document.body.innerText")));
        check_container_layout(&page);
        check_cancel_and_submit(&page, &add);
        page.locator("#open-count")
            .expect()
            .to_have_text("1 open todo")
            .unwrap_or_else(|e| panic!("{e:?}; {:?}", page.eval("document.body.innerText")));
        page.locator("#notification")
            .expect()
            .to_have_text("Todo added")
            .unwrap();
        shared_count.expect().to_have_text("1 open todo").unwrap();
        other
            .locator("#notification")
            .expect()
            .to_have_text("")
            .unwrap();
        let toggle = page.locator(&button("server:toggle_todo"));
        toggle.expect().to_have_text("Complete").unwrap();
        capture_page(&page, "list");
        assert_eq!(
            page.eval(
                r##"(() => {
                const title = document.querySelector(".ewe-text.text-primary");
                const style = getComputedStyle(title);
                return [style.color, style.paddingLeft].join("|");
            })()"##
            )
            .unwrap()
            .as_str(),
            if page
                .eval("matchMedia('(prefers-color-scheme: dark)').matches")
                .unwrap()
                .as_bool()
                == Some(true)
            {
                Some("rgb(154, 228, 216)|8px")
            } else {
                Some("rgb(21, 95, 89)|8px")
            }
        );
        assert_eq!(
            page.eval(
                r##"(() => {
                const title = document.querySelector(".ewe-text.text-primary");
                document.documentElement.style.setProperty("--spacing-md", "17px");
                document.documentElement.style.setProperty("--color-primary", "#663399");
                return [getComputedStyle(title).color, getComputedStyle(title).paddingLeft,
                    title === document.querySelector(".ewe-text.text-primary")].join("|");
            })()"##
            )
            .unwrap()
            .as_str(),
            Some("rgb(102, 51, 153)|17px|true")
        );
        shared_count.expect().to_have_text("1 open todo").unwrap();
        peer_toggle.expect().to_have_text("Complete").unwrap();
        other.locator("button[value=done]").click().unwrap();
        peer_toggle.expect().to_have_count(0).unwrap();
        let peer_add = other.locator(&button("host:open_add_dialog"));
        peer_add.click().unwrap();
        let peer_sheet = other.locator("dialog:modal input");
        peer_sheet.expect().to_be_visible().unwrap();
        peer_sheet.fill("Keep this draft").unwrap();
        submit_with_keyboard(&page, &add, "second todo");
        shared_count.expect().to_have_text("2 open todos").unwrap();
        peer_sheet.expect().to_be_visible().unwrap();
        assert_eq!(
            other
                .eval("document.querySelector('dialog:modal input').value")
                .unwrap()
                .as_str(),
            Some("Keep this draft")
        );
        other
            .locator("dialog:modal .ewe-cancel-button")
            .click()
            .unwrap();
        peer_toggle.expect().to_have_count(0).unwrap();
        let second_id = page.eval(r"Array.from(document.querySelectorAll('.ewe-checklist-row')).find(row => row.querySelector('.ewe-text')?.textContent === 'second todo').querySelector('button[value]').value").unwrap();
        let second_id: ui_todo::TodoId = second_id.as_str().unwrap().parse().unwrap();
        let second_delete = page.locator(&format!(
            r#"button[primal\:onclick="host:open_delete_dialog"][value="{second_id}"]:not([hidden] *)"#,
        ));
        second_delete.expect().to_be_visible().unwrap();
        second_delete.click().unwrap();
        let confirm_second = page.locator(&button("host:submit_delete"));
        confirm_second.expect().to_be_visible().unwrap();
        confirm_second.click().unwrap();
        page.locator(&button("host:submit_delete"))
            .expect()
            .to_have_count(0)
            .unwrap();
        shared_count.expect().to_have_text("1 open todo").unwrap();
        page.locator("button[value=done]").click().unwrap();
        toggle.expect().to_have_count(0).unwrap();
        page.locator("button[value=all]").click().unwrap();
        toggle.expect().to_be_visible().unwrap();
        let wire = serde_json::to_string(include_str!(
            "../../../backends/foundation_wasm_ui/runtimes/foundation-ui-wire.js"
        ))
        .unwrap();
        let install_wire = format!(
            r"(async () => {{
                const blob = new Blob([{wire}], {{type: 'text/javascript'}});
                const url = URL.createObjectURL(blob);
                try {{
                    window.testWire = await import(url);
                }} finally {{
                    URL.revokeObjectURL(url);
                }}
                return true;
            }})()"
        );
        other.eval(&install_wire).unwrap();
        other
            .eval(
                r"{
            const fetch = window.fetch.bind(window);
            window.fetch = (url, options) => {
                if (String(url).includes('/live')) window.replayUrl = String(url);
                if (String(url).includes('/live') && options?.method === 'POST') {
                    const {Frame, ByteBuffer, FrameRequest} = window.testWire;
                    const frame = Frame.getRootAsFrame(new ByteBuffer(options.body));
                    window.replayCursor = String(frame.kind(new FrameRequest()).resume().after());
                }
                const pending = fetch(url, options);
                if (String(url).includes('/live')) pending.then(r => window.replayStatus = r.status, e => window.replayStatus=String(e));
                return pending;
            };
        }",
            )
            .unwrap();
        let initial_loads = "performance.getEntriesByType('resource').filter(e => new URL(e.name).pathname === '/initial').length";
        let loads_before = other.eval(initial_loads).unwrap();
        channel.close();
        toggle.click().unwrap();
        shared_count
            .expect()
            .timeout(std::time::Duration::from_secs(20))
            .to_have_text("0 open todos")
            .unwrap_or_else(|err| {
                panic!(
                    "{err:?}; primary {:?}; observer {:?}",
                    page.eval("document.body.innerText"),
                    other.eval("[document.body.innerText,window.replayUrl,window.replayStatus]")
                )
            });
        assert_eq!(other.eval(initial_loads).unwrap(), loads_before);
        assert_eq!(
            other.eval("window.replayCursor").unwrap().as_str(),
            Some("3")
        );
        page.locator("#open-count")
            .expect()
            .to_have_text("0 open todos")
            .unwrap();
        peer_toggle.expect().to_have_text("Reopen").unwrap();
        other
            .locator(&button("host:open_delete_dialog"))
            .click()
            .unwrap();
        let confirm = other.locator(&button("host:submit_delete"));
        confirm.expect().to_be_visible().unwrap();
        capture_page(&other, "confirmation");
        confirm.expect().to_be_visible().unwrap();
        other.eval(r"window.staleTodoButton = Array.from(document.querySelectorAll('button')).find(el => el.getAttribute('primal:onclick') === 'host:submit_delete').cloneNode(true); true").unwrap();
        confirm.click().unwrap();
        toggle.expect().to_have_count(0).unwrap();
        peer_toggle.expect().to_have_count(0).unwrap();
        other.eval(r"{
            // A repeated deletion succeeds. An unknown record still returns an error.
            window.staleTodoButton.value = '00000000-0000-4000-8000-999999999999';
            document.querySelector('#page').appendChild(window.staleTodoButton);
            window.FoundationWasmUiRuntime.Patcher.runtime.dispatcher.scanAndWire(window.staleTodoButton);
            window.staleTodoButton.click();
        }").unwrap();
        other
            .locator("#notice")
            .expect()
            .to_have_text("ActionFailed")
            .unwrap();
        // A separate page sends typed Connect requests through the browser's Fetch client.
        let rpc = driver.new_page().unwrap();
        open_document(&rpc, &format!("{url}?rpc"));
        let open = rpc.locator(&button("host:open_add_dialog"));
        open.expect().to_be_visible().unwrap_or_else(|e| {
            panic!(
                "{e:?}; {:?}",
                rpc.eval("document.querySelector('#notice').value")
            )
        });
        assert_eq!(
            rpc.eval(r#"performance.getEntriesByType("resource").map(e => new URL(e.name).pathname).filter(p => p.endsWith(".js")).join(",")"#).unwrap().as_str(),
            Some("/pkg/bundle.js")
        );
        assert_eq!(
            rpc.eval(r#"performance.getEntriesByType("resource").filter(e => new URL(e.name).pathname === "/pkg/rpc/foundation_ui_connectrpc_bg.wasm").length"#).unwrap().as_u64(),
            Some(1)
        );
        let calls = "performance.getEntriesByType('resource').filter(e => new URL(e.name).pathname === '/ewe.ui.v1.Actions/Exchange').length";
        rpc.eval("(async () => { while (!window.todoReady) await new Promise(requestAnimationFrame); })()")
            .unwrap();
        let start = rpc.eval(calls).unwrap().as_u64().unwrap();
        assert!(start >= 1);
        // Open the dialog without changing tab visibility or refreshing the countdown.
        let selector = serde_json::json!(button("host:open_add_dialog"));
        rpc.eval(&format!("document.querySelector({selector}).click()"))
            .unwrap();
        let input = rpc.locator("dialog:modal input");
        input.expect().to_be_visible().unwrap();
        assert_eq!(rpc.eval(calls).unwrap().as_u64(), Some(start));
        input.fill("RPC task").unwrap();
        let before_submit = rpc.eval(calls).unwrap().as_u64().unwrap();
        input.press("Enter").unwrap();
        rpc.locator("#open-count")
            .expect()
            .to_have_text("1 open todo")
            .unwrap();
        rpc.locator(&button("server:toggle_todo"))
            .expect()
            .to_be_visible()
            .unwrap();
        assert_eq!(rpc.eval(calls).unwrap().as_u64(), Some(before_submit + 1));
        open_document(&rpc, "about:blank");
        let starter = driver.new_page().unwrap();
        open_document(&starter, &format!("{url}?count"));
        starter
            .locator("#open-count")
            .expect()
            .to_have_text("1")
            .unwrap();
        let refresh = starter.locator(&button("server:refresh_count"));
        refresh.expect().to_be_visible().unwrap();
        let socket = driver.new_page().unwrap();
        open_document(&socket, &format!("{url}?socket"));
        let open = socket.locator(&button("host:open_add_dialog"));
        open.expect().to_be_visible().unwrap_or_else(|e| {
            panic!(
                "{e:?}; {:?}",
                socket.eval("document.querySelector('#notice').value")
            )
        });
        submit_with_keyboard(&socket, &open, "Socket task");
        socket
            .locator("#open-count")
            .expect()
            .to_have_text("2 open todos")
            .unwrap();
        shared_count.expect().to_have_text("2 open todos").unwrap();
        refresh.click().unwrap();
        starter
            .locator("#open-count")
            .expect()
            .to_have_text("2")
            .unwrap();
        for finished in [&page, &other, &socket, &starter] {
            open_document(finished, "about:blank");
        }
        let external = driver.new_page().unwrap();
        open_document(&external, &format!("{url}theme.css"));
        assert_eq!(
            external.eval(&lifecycle_script()).unwrap(),
            serde_json::json!([2, 2, 2])
        );
    });
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
    capture_page(page, "entry");
    page.locator("dialog:modal .ewe-submit-button")
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

fn capture_page(page: &foundation_browser::Page<'_>, state: &str) {
    if let Some(output) = std::env::var_os("EWE_WEB_CAPTURES") {
        let output = std::path::PathBuf::from(output);
        std::fs::create_dir_all(&output).unwrap();
        page.screenshot(output.join(format!("web-{state}.png")))
            .unwrap();
    }
}

/// Run with two native peers on the same server. The external test driver publishes one update.
#[test]
#[ignore = "requires mobile peers and a live server"]
fn one_count_publication_reaches_the_open_browser() {
    let base = std::env::var("EWE_SHARED_COUNT_URL").unwrap();
    let before = std::env::var("EWE_SHARED_COUNT_BEFORE").unwrap();
    let after = std::env::var("EWE_SHARED_COUNT_AFTER").unwrap();
    let driver = BrowserDriver::launch(LaunchConfig::default().ctx(Ctx::host())).unwrap();
    let page = driver.new_page().unwrap();
    page.goto(&base).unwrap();
    let count = page.locator("#open-count");
    count.expect().to_have_text(&before).unwrap();
    println!("Browser subscribed at {before}");
    count
        .expect()
        .timeout(std::time::Duration::from_secs(90))
        .to_have_text(&after)
        .unwrap();
}

fn lifecycle_script() -> String {
    use std::{fs, path::Path};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    foundation_wasm_ui::build_hook::application_sources(root).unwrap();
    fs::create_dir(root.join("bindings")).unwrap();
    fs::copy(
        Path::new(env!("OUT_DIR")).join("browser-bindings/ui_todo_bg.js"),
        root.join("bindings/ui_todo_bg.js"),
    )
    .unwrap();
    let entry = root.join("lifecycle.js");
    fs::write(&entry, include_str!("browser_app_lifecycle.js")).unwrap();
    let mut script = String::from_utf8(
        foundation_codegen::javascript::application(&entry, root, false)
            .unwrap()
            .bytes,
    )
    .unwrap();
    script.push_str("\nglobalThis.todoLifecycleCheck;");
    script
}

fn game_test_bindings() -> String {
    use std::{io::Write, path::Path};
    let root = Path::new(env!("OUT_DIR")).join("browser-bindings");
    let mut entry = tempfile::Builder::new()
        .suffix(".js")
        .tempfile_in(&root)
        .unwrap();
    entry
        .write_all(
            br#"
        import initialize, * as game from "./mathalley/ui_todo.js";
        globalThis.todoTestGameBindings = async () => {
            await initialize({module_or_path: await window.gameTestBinary});
            return game;
        };
    "#,
        )
        .unwrap();
    let script = foundation_codegen::javascript::application(entry.path(), &root, false).unwrap();
    String::from_utf8(script.bytes).unwrap() + "\ntrue"
}

#[test]
fn game_view_keeps_the_todo_subscription_and_draws_on_the_host_device() {
    browser_server::with_browser_flags(
        &["--enable-gpu", "--enable-unsafe-webgpu"],
        |driver, url, _| {
            let peer = driver.new_page().unwrap();
            open_document(&peer, url);
            let page = driver.new_page().unwrap();
            open_document(&page, url);
            let add = r#"button[primal\:onclick="host:open_add_dialog"]"#;
            page.locator(add).expect().to_be_visible().unwrap();
            peer.locator(add).expect().to_be_visible().unwrap();
            submit_with_keyboard(&page, &page.locator(add), "First game task");
            page.locator("button[value=open]").click().unwrap();
            page.locator("#open-count")
                .expect()
                .to_have_text("1 open todo")
                .unwrap();
            page.eval(include_str!("mathalley/host-setup.js")).unwrap();
            for pass in 0..2 {
                page.locator(r#"button[primal\:onclick="host:open_mathalley"]"#)
                    .click()
                    .unwrap();
                let canvas = page.locator(r#"#mathalley[aria-busy="false"] canvas"#);
                canvas.expect().to_be_visible().unwrap_or_else(|e| {
                    panic!("{e:?}: {:?}", page.eval("document.body.innerText"))
                });
                assert_eq!(
                page.eval(r#"performance.getEntriesByType("resource").map(e => new URL(e.name).pathname).filter(p => p.endsWith(".js") || p.includes("/pkg/mathalley/")).join(",")"#).unwrap().as_str(),
                Some("/pkg/bundle.js,/pkg/mathalley/ui_todo_bg.wasm")
            );
                if pass == 0 {
                    page.eval(&game_test_bindings()).unwrap();
                }
                canvas.click().unwrap();
                let capture = std::env::temp_dir()
                    .join(format!("mathalley-presented-{}.png", std::process::id()));
                page.screenshot(&capture).unwrap();
                let png = std::fs::read(&capture).unwrap();
                std::fs::remove_file(capture).unwrap();
                page.eval(&format!(
                    "window.gamePresentedPng=new Uint8Array({png:?});true"
                ))
                .unwrap();
                page.eval(include_str!("mathalley/host-device.js")).unwrap();
                if pass == 0 {
                    page.eval(include_str!("mathalley/sound-views.js")).unwrap();
                    play_both_games(&page);
                    submit_with_keyboard(&peer, &peer.locator(add), "Second game task");
                    page.locator("#open-count")
                        .expect()
                        .to_have_text("2 open todos")
                        .unwrap();
                }
                page.locator("#mathalley button").click().unwrap();
                page.locator("button[value=open]")
                    .expect()
                    .to_be_visible()
                    .unwrap();
                page.locator("#open-count")
                    .expect()
                    .to_have_text("2 open todos")
                    .unwrap();
                capture_page(&page, "mathalley-return");
            }
        },
    );
}

/// Use the same seed and clock on the reference simulation and the browser input surface.
fn play_both_games(page: &foundation_browser::Page<'_>) {
    use ui_todo::mathalley::{
        game::{Action, Game, LevelState, Phase},
        scene,
    };
    let mut model = Game::new(13);
    model.apply(Action::Start);
    let mut time = 1001;
    model.update(std::time::Duration::from_millis(time));
    let mut saw_gumball = false;
    for _ in 0..32 {
        let (name, index) = match &model.level {
            LevelState::Train(train) => ("Train", train.correct),
            LevelState::Gumball(level) => ("Gumball", level.correct),
            LevelState::Empty => panic!("A playing level has a game"),
        };
        page.eval(&format!("window.gameTestTime={time}; new Promise(done=>window.gameTestFrame(()=>window.gameTestFrame(done)))")).unwrap();
        page.locator(&format!(r#"canvas[aria-label="Mathalley {name}"]"#))
            .expect()
            .to_be_visible()
            .unwrap();
        if name == "Gumball" {
            capture_page(page, "gumball");
        }
        let hit = scene::build(&model, true)
            .hits
            .into_iter()
            .find(|h| h.action == Action::Choose(index))
            .unwrap();
        page.eval(&format!(r#"(() => {{
            const c=document.querySelector('#mathalley canvas'), b=c.getBoundingClientRect();
            const scale=Math.min(b.width/390,b.height/844);
            c.dispatchEvent(new PointerEvent('pointerdown', {{bubbles:true,clientX:b.x+(b.width-390*scale)/2+{}*scale,clientY:b.y+(b.height-844*scale)/2+{}*scale}}));
            return true;
        }})()"#,hit.x + hit.w / 2.0,hit.y + hit.h / 2.0)).unwrap();
        model.apply(Action::Choose(index));
        if name == "Gumball" {
            saw_gumball = true;
            break;
        }
        time += 1001;
        model.update(std::time::Duration::from_millis(time));
        if model.phase == Phase::Intro {
            page.eval(&format!("window.gameTestTime={time};new Promise(done=>{{function wait(){{if(window.lastGameTick==={time})done(true);else window.gameTestFrame(wait);}}wait();}})")).unwrap();
            time += 1001;
            model.update(std::time::Duration::from_millis(time));
        }
        assert_eq!(model.phase, Phase::Playing);
    }
    assert!(saw_gumball);
}
