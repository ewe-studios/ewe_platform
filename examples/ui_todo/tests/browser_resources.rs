//! Read the installed browser resource module through the Todo server.
use foundation_browser::{BrowserDriver, LaunchConfig};
use foundation_context::Ctx;
use foundation_http::ShutdownSignal;
use foundation_wasm_ui::server::{BacklogConfig, LiveChannel};
use std::{net::TcpListener, path::Path, sync::Arc};

#[test]
fn browser_opens_the_same_packaged_icon() {
    let web = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/web");
    assert!(
        web.join("resources-wasm/foundation_resources.js").is_file(),
        "Run build-module.sh before this test"
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/art/todo.svg", listener.local_addr().unwrap());
    let shutdown = Arc::new(ShutdownSignal::new());
    let stop = shutdown.clone();
    let worker = std::thread::spawn(move || {
        let channel = LiveChannel::new(
            1,
            BacklogConfig::default(),
            foundation_ui_wire::writer::WriterConfig::default(),
        );
        let app = ui_todo::server::app_with_public_directory(
            &web.join("ui_todo.wasm"),
            channel,
            Path::new(env!("EWE_STAGED_RESOURCES")),
        )
        .unwrap();
        app.server(&listener.local_addr().unwrap().to_string())
            .serve_with_listener(&listener, &stop);
    });
    let result = std::panic::catch_unwind(|| {
        let driver = BrowserDriver::launch(LaunchConfig::default().ctx(Ctx::host())).unwrap();
        let page = driver.new_page().unwrap();
        page.goto(&url).unwrap();
        let result = page.eval(r#"(async () => {
            const { default: initialize, BrowserResources } = await import("/resources-wasm/foundation_resources.js");
            await initialize();
            const resources = new BrowserResources(location.origin + "/");
            const file = await resources.open("art/todo.svg");
            const decoder = new TextDecoder();
            let icon = "", chunk;
            while ((chunk = await file.nextChunk()) !== undefined) icon += decoder.decode(chunk, {stream: true});
            file.free();
            for (const [name, code] of [["invalid/../name", "InvalidName"], ["absent.txt", "Missing"]]) {
                try { await resources.open(name); throw Error("Expected " + code); }
                catch (e) { if (e.name !== code) throw e; }
            }
            const originalFetch = window.fetch;
            let sent = false, cancelled = false;
            try {
                window.fetch = async (url) => {
                    if (new URL(url).pathname === "/unavailable") return new Response(null, {status: 503});
                    return new Response(new ReadableStream({
                        pull(controller) {
                            if (sent) controller.error(new Error("Transfer interrupted"));
                            else { sent = true; controller.enqueue(new Uint8Array([42])); }
                        },
                        cancel() { cancelled = true; }
                    }, {highWaterMark: 0}));
                };
                try { await resources.open("unavailable"); throw Error("Expected HTTP error"); }
                catch (e) { if (e.name !== "Read") throw e; }
                const interrupted = await resources.open("interrupted");
                if ((await interrupted.nextChunk())[0] !== 42) throw Error("Missing initial data");
                try { await interrupted.nextChunk(); throw Error("Expected stream error"); }
                catch (e) { if (e.name !== "Read") throw e; }
                interrupted.free();
                const unused = await resources.open("unused");
                unused.free();
                if (!cancelled) throw Error("Closing the handle did not cancel the body");
            } finally { window.fetch = originalFetch; }
            resources.free();
            return icon + decoder.decode();
        })()"#).unwrap();
        assert_eq!(
            result.as_str(),
            Some(include_str!("../resources/art/todo.svg"))
        );
    });
    shutdown.turn_on();
    worker.join().unwrap();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic)
    }
}
