//! A Todo server and a fresh Chromium instance for one integration test.
use foundation_browser::{BrowserDriver, LaunchConfig};
use foundation_context::Ctx;
use foundation_http::ShutdownSignal;
use foundation_ui_wire::writer::WriterConfig;
use foundation_wasm_ui::server::{BacklogConfig, LiveChannel};
use std::{net::TcpListener, path::Path, sync::Arc};

pub fn with_browser_server(
    check: impl FnOnce(&BrowserDriver<'_>, &str, &LiveChannel) + std::panic::UnwindSafe,
) {
    let module = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/web/ui_todo.wasm");
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
        let app = ui_todo::server::app_with_public_directory(
            &module,
            served,
            Path::new(env!("EWE_STAGED_RESOURCES")),
        )?;
        app.server(&listener.local_addr()?.to_string())
            .serve_with_listener(&listener, &stop);
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    });
    let outcome = std::panic::catch_unwind(|| {
        // Both test tabs must deliver stream events while Chrome displays the other tab.
        let config = LaunchConfig {
            extra_args: vec![
                "--disable-background-timer-throttling".into(),
                "--disable-popup-blocking".into(),
                "--disable-renderer-backgrounding".into(),
                "--disable-backgrounding-occluded-windows".into(),
            ],
            ..LaunchConfig::default().ctx(Ctx::host())
        };
        let driver = BrowserDriver::launch(config).unwrap();
        check(&driver, &url, &channel);
    });
    shutdown.turn_on();
    worker.join().unwrap().unwrap();
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic)
    }
}

/// Wait for this tab's next document. Each live tab has its own load events.
pub fn open_document(page: &foundation_browser::Page<'_>, url: &str) {
    let address = serde_json::json!(url);
    page.eval(&format!(
        "document.documentElement.dataset.leaving = 'yes'; location.assign({address})"
    ))
    .unwrap();
    let ready = if url.ends_with("/theme.css") {
        "html:not([data-leaving]) pre"
    } else if url == "about:blank" {
        "html:not([data-leaving]) body"
    } else {
        "html:not([data-leaving]) #open-count"
    };
    page.locator(ready)
        .expect()
        .timeout(LaunchConfig::default().navigate_timeout)
        .to_have_count(1)
        .unwrap_or_else(|error| {
            panic!(
                "Opening {url}: {error:?}, {:?}",
                page.eval("[location.href, document.readyState, document.body?.innerText]")
            )
        });
}
