//! The mobile responders use the one shared MIME table.
//!
//! WHY: a second hand-written map served `.mjs` as HTML (issue #592).
//! WHAT: every extension resolves through `foundation_netio::shared::mime`.
//! HOW: serve a file and check the `CONTENT_TYPE` header.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use foundation_nostd::embeddable::FileInfo;
use foundation_nostd::mobile::MobileDirectory;
use foundation_platform::*;
use tauri::http::header;

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A disk whose root is a temporary directory.
struct TempDisk {
    root: String,
    dir: PathBuf,
}

impl MobileDirectory for TempDisk {
    const FILES_METADATA: &'static [FileInfo] = &[];

    fn root_str(&self) -> &str {
        &self.root
    }
}

impl Drop for TempDisk {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn disk(files: &[(&str, &str)]) -> TempDisk {
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "foundation_platform_mime_{}_{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the temporary root");
    for (name, body) in files {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("the file parent");
        }
        std::fs::write(path, body).expect("the test file");
    }
    TempDisk {
        root: dir.to_string_lossy().into_owned(),
        dir,
    }
}

fn session() -> Arc<PlatformSession> {
    PlatformSession::new_test(
        PathBuf::from("."),
        foundation_context::Ctx::fixed(Arc::new(foundation_context::MonotonicClock::new())),
    )
}

fn intent(url: &str) -> NavigationIntent {
    NavigationIntent {
        url: url.to_string(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    }
}

fn content_type(url: &str, files: &[(&str, &str)]) -> String {
    let app = MobileApp::new(disk(files));
    let response = app.respond(&intent(url), &webview_app(), &session());
    response.headers()[header::CONTENT_TYPE]
        .to_str()
        .expect("an ASCII content type")
        .to_string()
}

#[test]
fn a_module_script_is_javascript() {
    assert_eq!(
        content_type("/app/main.mjs", &[("app/main.mjs", "export {};")]),
        "text/javascript"
    );
}

#[test]
fn a_classic_script_is_javascript() {
    assert_eq!(
        content_type("/app/main.js", &[("app/main.js", "void 0;")]),
        "text/javascript"
    );
}

#[test]
fn html_keeps_the_utf8_parameter() {
    assert_eq!(
        content_type("/app/index.html", &[("app/index.html", "<p>hello</p>")]),
        "text/html; charset=utf-8"
    );
}

#[test]
fn wasm_is_wasm() {
    assert_eq!(
        content_type("/app/mod.wasm", &[("app/mod.wasm", "\0asm")]),
        "application/wasm"
    );
}

#[test]
fn an_unknown_name_falls_back_to_html() {
    assert_eq!(
        content_type("/app/data.unknown", &[("app/data.unknown", "x")]),
        "text/html; charset=utf-8"
    );
}
