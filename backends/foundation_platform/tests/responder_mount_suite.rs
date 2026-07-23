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
use foundation_platform::pattern::PatternRouter;
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

/// The decision the **router** produces for `pattern` given `url`.
///
/// Going through `PatternRouter` rather than hand-building a decision is the
/// point: `sub_path` is what the responder now reads, and only the router
/// fills it in. A hand-made decision would test nothing about how a request
/// actually reaches an app.
fn routed(pattern: &str, url: &str) -> RouteDecision {
    let mut router = PatternRouter::new();
    router.route(pattern, webview_app());
    // `PlatformSession::register_route_with` also registers the bare prefix,
    // because `/app/*` requires a segment after the slash and so cannot match
    // a bare `/app/`. Mirror that here or the directory request 404s in the
    // test while working in production.
    if let Some(prefix) = pattern.strip_suffix("/*") {
        router.route(prefix, webview_app());
    }
    router
        .resolve_intent(&intent(url))
        .unwrap_or_else(|| panic!("{pattern} must match {url}"))
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

    let response = responder.respond(&intent("/app/index.html"), &routed("/app/*", "/app/index.html"), &session);

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
        let response = responder.respond(&intent(url), &routed("/app/*", url), &session);
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
        &routed("/app/*", "/app/settings/profile"),
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
        &routed("/app-hello/*", "/app-hello/index.html"),
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
        &routed("/app/*", "/app/missing.wasm"),
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
        let response = responder.respond(&intent(url), &routed("/app/*", url), &session);
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

    let before = responder.respond(&intent("/app/"), &routed("/app/*", "/app/"), &session);
    assert_eq!(body_of(&before), "<html>old</html>");

    manager.activate("app", "0.1.1").expect("activate");

    let after = responder.respond(&intent("/app/"), &routed("/app/*", "/app/"), &session);
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
    let response = responder.respond(&intent("/app/index.html"), &routed("/app/*", "/app/index.html"), &session);

    assert_eq!(response.status(), 200, "body was: {}", body_of(&response));
    assert_eq!(body_of(&response), "<html>disk</html>");
}

// ── The router owns where a route's prefix ends ─────────────────────────
//
// `sub_path` is the seam. A responder must not re-derive it: the router
// matched the pattern segment by segment and already knows the answer, and a
// second implementation is free to disagree — which is precisely how
// `"app-hello".strip_prefix("app")` shipped.

#[test]
#[traced_test]
fn the_router_reports_the_path_relative_to_a_routes_literal_prefix() {
    for (pattern, url, expected) in [
        ("/app/*", "/app/index.html", "index.html"),
        ("/app/*", "/app/nested/deep/bundle.js", "nested/deep/bundle.js"),
        ("/app/*", "/app/", ""),
        ("/app/*", "/app", ""),
        ("/app-hello/*", "/app-hello/index.html", "index.html"),
        ("/api/system", "/api/system", ""),
    ] {
        assert_eq!(
            routed(pattern, url).sub_path.as_deref(),
            Some(expected),
            "{pattern} + {url}"
        );
    }
}

#[test]
#[traced_test]
fn a_route_prefix_never_matches_a_longer_sibling_segment() {
    let mut router = PatternRouter::new();
    router.route("/app/*", webview_app());
    router.route("/app", webview_app());

    assert!(
        router.resolve_intent(&intent("/app-hello/index.html")).is_none(),
        "the router matches whole segments, so `app` cannot claim `app-hello` \
         — this is why the responder must not do its own prefix arithmetic"
    );
}

#[test]
#[traced_test]
fn each_app_gets_the_same_relative_path_from_its_own_route() {
    // Two apps, two routes, one file name. Each responder sees `index.html`
    // — no responder needs to know its own prefix to get there.
    assert_eq!(
        routed("/app/*", "/app/index.html").sub_path,
        routed("/app-hello/*", "/app-hello/index.html").sub_path,
    );
}
