//! Platform Android — WASM UI app.
//!
//! All WASM entrypoints live here. `#[wasm_bin(extern = "true")]` auto-generates
//! `#[no_mangle] pub extern "C"` — zero boilerplate for export functions.
//!
//! Discovered by root `build.rs` source scanner. Compiled to wasm32-unknown-unknown.
//! Each annotation produces a separate JS wrapper. All exports share one `.wasm` binary.

use foundation_macros::{wasm_bin, wasm_service, wasm_worker};
use foundation_wasm_ui::html;
use foundation_wasm_ui::App;

// ── #[wasm_bin] — Main thread WASM app ──────────────────────────

/// Renders the foundation_platform dashboard in the WebView.
/// `extern = "true"` → `#[no_mangle] pub extern "C" fn platform_dashboard()`.
#[wasm_bin(extern = "true")]
fn platform_dashboard() {
    let app = App::new();
    let (ctx, rcv) = app.context();

    let dashboard = html! { ctx, rcv,
        <div style="padding:16px;font-family:sans-serif;background:#0a0a1a;color:#ccd6f6">
            <h1 style="color:#64ffda;font-size:22px;margin:0 0 4px">"Foundation Platform"</h1>
            <p style="color:#8892b0;font-size:12px;margin:0 0 16px">"Android — WASM UI · columnar v1"</p>

            <div style="display:flex;gap:6px;flex-wrap:wrap;margin:0 0 12px">
                <a href="ewe://localhost/app/home"
                   style="padding:8px 14px;background:#112240;color:#64ffda;border:1px solid #233554;border-radius:6px;text-decoration:none;font-size:13px">"🏠 App (WASM)"</a>
                <a href="ewe://localhost/api/status"
                   style="padding:8px 14px;background:#112240;color:#64ffda;border:1px solid #233554;border-radius:6px;text-decoration:none;font-size:13px">"⚡ API (IPC)"</a>
                <a href="ewe://localhost/remote/news"
                   style="padding:8px 14px;background:#112240;color:#64ffda;border:1px solid #233554;border-radius:6px;text-decoration:none;font-size:13px">"🌐 Remote"</a>
            </div>

            <div style="background:#112240;border:1px solid #233554;border-radius:8px;padding:10px;margin:8px 0">
                <h3 style="color:#64ffda;font-size:13px;margin:0 0 6px">"Session"</h3>
                <p style="color:#8892b0;font-size:12px;margin:0">"PlatformSession via platform_run! + WASM UI"</p>
            </div>
            <p style="color:#445566;font-size:10px;margin:12px 0 0">"foundation_platform 0.0.1"</p>
        </div>
    };

    let _root_id = app.mount(dashboard);
    app.stabilize();
    core::mem::forget(app);
}

// ── #[wasm_worker] — Web Worker (off-main-thread) ───────────────

/// Worker for background computation. Same `.wasm`, separate JS wrapper.
#[wasm_worker(extern = "true")]
fn bg_worker() {
    // Post-MVP: receive messages from main thread, process, post results back.
}

// ── #[wasm_service] — Service Worker (fetch interception) ───────

/// Service worker for offline caching and fetch routing.
#[wasm_service(routes = ["/app/*", "/remote/*", "/cached/*"], extern = "true")]
fn platform_sw() {
    // Post-MVP: intercept fetch events, serve from cache,
    // route platform:// requests through the session backbone.
}
