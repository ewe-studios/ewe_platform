//! F40 — `MobileApp` mounting and path resolution.
//!
//! This is where the Android bug actually surfaced: a request arrives as
//! `/app/index.html`, the responder's root already *is* the app directory,
//! and the bytes live inside the APK rather than on disk. Get any of the
//! three wrong and the page 404s, which is exactly what shipped before F40.

use std::path::PathBuf;
use std::sync::Arc;

use foundation_nativeapis::shared::vfs::dynfs::DynFs;
use foundation_nativeapis::shared::vfs::memory_fs::MemoryFs;
use foundation_platform::assets::{AssetLayout, PlatformAssetManager};
use foundation_platform::{webview_app, MobileApp, PlatformSession, RouteResponder};
use foundation_nostd::embeddable::FileInfo;
use foundation_nostd::mobile::MobileDirectory;
use foundation_ui_traits::{IntentSource, Method, NavigationIntent, RouteDecision};
use tracing_test::traced_test;

// ── Fixtures ────────────────────────────────────────────────────────────

/// A `MobileDirectory` with no compile-time metadata — everything this suite
/// asserts is resolved at runtime, which is the part F40 changed.
struct Assets {
    root: PathBuf,
}

impl MobileDirectory for Assets {
    const FILES_METADATA: &'static [FileInfo] = &[];
    fn root_str(&self) -> &str {
        self.root.to_str().unwrap_or("")
    }
}

fn intent(url: &str) -> NavigationIntent {
    NavigationIntent {
        url: format!("ewe://localhost{url}"),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    }
}

/// The decision an app route carries. Its fields are irrelevant here —
/// `MobileApp` resolves purely from the intent URL and its mount — but the
/// responder signature requires one.
fn decision() -> RouteDecision {
    webview_app()
}

/// A session with a Versioned asset manager holding one app's bundle.
fn session_with_bundle(files: &[(&str, &str)]) -> (Arc<PlatformSession>, Arc<PlatformAssetManager>) {
    let manager = Arc::new(PlatformAssetManager::from_vfs(
        DynFs::new(Arc::new(MemoryFs::new())),
        PathBuf::from("/base"),
        "0.1.0",
        AssetLayout::Versioned,
        None,
        None,
    ));
    for (path, body) in files {
        manager.write(path, body.as_bytes()).expect("seed bundle");
    }

    let session = PlatformSession::new_test(PathBuf::from("/base"));
    session.set_asset_manager(Arc::clone(&manager));
    (session, manager)
}

fn body_of(response: &tauri::http::Response<Vec<u8>>) -> String {
    String::from_utf8_lossy(response.body()).to_string()
}

// ── Mounted resolution ──────────────────────────────────────────────────

#[test]
#[traced_test]
fn a_mounted_responder_strips_its_own_app_segment() {
    let (session, _) = session_with_bundle(&[("/app/v0.1.0/index.html", "<html>app</html>")]);
    let responder = MobileApp::mounted_at(
        Assets { root: PathBuf::from("/base/app/v0.1.0") },
        "app",
    );

    let response = responder.respond(&intent("/app/index.html"), &decision(), &session);

    assert_eq!(response.status(), 200, "body was: {}", body_of(&response));
    assert_eq!(
        body_of(&response),
        "<html>app</html>",
        "without stripping this would look for app/v0.1.0/app/index.html"
    );
}

#[test]
#[traced_test]
fn a_directory_request_serves_the_apps_index() {
    let (session, _) = session_with_bundle(&[("/app/v0.1.0/index.html", "<html>app</html>")]);
    let responder = MobileApp::mounted_at(
        Assets { root: PathBuf::from("/base/app/v0.1.0") },
        "app",
    );

    for url in ["/app/", "/app"] {
        let response = responder.respond(&intent(url), &decision(), &session);
        assert_eq!(response.status(), 200, "{url} gave: {}", body_of(&response));
        assert_eq!(body_of(&response), "<html>app</html>", "for {url}");
    }
}

#[test]
#[traced_test]
fn an_unknown_sub_route_falls_back_to_the_apps_index_for_client_side_routing() {
    let (session, _) = session_with_bundle(&[("/app/v0.1.0/index.html", "<html>spa</html>")]);
    let responder = MobileApp::mounted_at(
        Assets { root: PathBuf::from("/base/app/v0.1.0") },
        "app",
    );

    let response = responder.respond(
        &intent("/app/settings/profile"),
        &decision(),
        &session,
    );

    assert_eq!(response.status(), 200);
    assert_eq!(
        body_of(&response),
        "<html>spa</html>",
        "an SPA route the server has never heard of must still boot the app"
    );
}

#[test]
#[traced_test]
fn a_sibling_app_prefix_is_not_mistaken_for_this_one() {
    let (session, _) = session_with_bundle(&[
        ("/app/v0.1.0/index.html", "<html>app</html>"),
        ("/app-hello/v0.1.0/index.html", "<html>hello</html>"),
    ]);
    // Mounted at "app", asked for "app-hello". Stripping a bare `app` prefix
    // would leave `-hello/index.html` and quietly serve the wrong bundle.
    let responder = MobileApp::mounted_at(
        Assets { root: PathBuf::from("/base/app/v0.1.0") },
        "app",
    );

    let response = responder.respond(
        &intent("/app-hello/index.html"),
        &decision(),
        &session,
    );

    assert_ne!(
        body_of(&response),
        "<html>hello</html>",
        "one app's responder must never serve another app's files"
    );
}

#[test]
#[traced_test]
fn a_missing_file_is_reported_as_not_found() {
    let (session, _) = session_with_bundle(&[("/app/v0.1.0/index.html", "<html>app</html>")]);
    let responder = MobileApp::mounted_at(
        Assets { root: PathBuf::from("/base/app/v0.1.0") },
        "app",
    );

    let response = responder.respond(
        &intent("/app/missing.wasm"),
        &decision(),
        &session,
    );

    assert_eq!(
        response.status(),
        404,
        "a concrete file with an extension must not silently fall back to index.html"
    );
}

#[test]
#[traced_test]
fn content_types_follow_the_extension() {
    let (session, _) = session_with_bundle(&[
        ("/app/v0.1.0/index.html", "<html/>"),
        ("/app/v0.1.0/bundle.js", "console.log(1)"),
        ("/app/v0.1.0/app.wasm", "\0asm"),
    ]);
    let responder = MobileApp::mounted_at(
        Assets { root: PathBuf::from("/base/app/v0.1.0") },
        "app",
    );

    for (url, expected) in [
        ("/app/index.html", "text/html; charset=utf-8"),
        ("/app/bundle.js", "application/javascript"),
        ("/app/app.wasm", "application/wasm"),
    ] {
        let response = responder.respond(&intent(url), &decision(), &session);
        assert_eq!(
            response.headers().get("content-type").and_then(|v| v.to_str().ok()),
            Some(expected),
            "for {url} — a wasm served as text/html will not instantiate"
        );
    }
}

// ── Version tracking ────────────────────────────────────────────────────

#[test]
#[traced_test]
fn a_mounted_responder_follows_its_app_to_a_new_version() {
    let (session, manager) = session_with_bundle(&[
        ("/app/v0.1.0/index.html", "<html>old</html>"),
        ("/app/v0.1.1/index.html", "<html>new</html>"),
    ]);
    let responder = MobileApp::mounted_at(
        Assets { root: PathBuf::from("/base/app/v0.1.0") },
        "app",
    );

    let before = responder.respond(&intent("/app/"), &decision(), &session);
    assert_eq!(body_of(&before), "<html>old</html>");

    manager.activate("app", "0.1.1").expect("activate");

    let after = responder.respond(&intent("/app/"), &decision(), &session);
    assert_eq!(
        body_of(&after),
        "<html>new</html>",
        "the responder resolves through the manager on every request, so an \
         OTA activation takes effect without rebuilding the route table"
    );
}

// ── Without a manager ───────────────────────────────────────────────────

#[test]
#[traced_test]
fn an_unmounted_responder_keeps_the_pre_f40_behaviour() {
    // No asset manager installed: paths resolve verbatim against the root,
    // which is what every embedding that predates F40 relies on.
    let session = PlatformSession::new_test(PathBuf::from("/base"));
    let dir = std::env::temp_dir().join("ewe_f40_unmounted");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("app")).expect("mkdir");
    std::fs::write(dir.join("app/index.html"), "<html>disk</html>").expect("write");

    let responder = MobileApp::new(Assets { root: dir.clone() });
    let response = responder.respond(&intent("/app/index.html"), &decision(), &session);

    assert_eq!(response.status(), 200, "body was: {}", body_of(&response));
    assert_eq!(body_of(&response), "<html>disk</html>");
}
