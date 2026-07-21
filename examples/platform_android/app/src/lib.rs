//! Platform Android — WASM UI app.

use foundation_macros::wasm_bin;
use foundation_wasm_ui::html;
use foundation_wasm_ui::App;

/// Renders the foundation_platform dashboard in the WebView.
/// Compiles to wasm32-unknown-unknown, loaded via bundle.js + WasmLoader.
#[wasm_bin(extern = "true")]
fn platform_dashboard() {
    let app = App::new();
    let (ctx, rcv) = app.context();

    let dashboard = html! { ctx, rcv,
        <div style="padding:16px;font-family:sans-serif;background:#0a0a1a;color:#ccd6f6">
            <h1 style="color:#64ffda;font-size:22px;margin:0 0 4px">"Foundation Platform"</h1>
            <p style="color:#8892b0;font-size:12px;margin:0 0 16px">"Android — WASM UI"</p>
            <div style="display:flex;gap:6px;flex-wrap:wrap;margin:0 0 12px">
                <a href="ewe://localhost/app/home"
                   style="padding:8px 14px;background:#112240;color:#64ffda;border:1px solid #233554;border-radius:6px;text-decoration:none;font-size:13px">"🏠 App"</a>
                <a href="ewe://localhost/app-hello/"
                   style="padding:8px 14px;background:#112240;color:#64ffda;border:1px solid #233554;border-radius:6px;text-decoration:none;font-size:13px">"👋 Hello"</a>
                <a href="ewe://localhost/api/status"
                   style="padding:8px 14px;background:#112240;color:#64ffda;border:1px solid #233554;border-radius:6px;text-decoration:none;font-size:13px">"⚡ API"</a>
                <a href="ewe://localhost/remote/news"
                   style="padding:8px 14px;background:#112240;color:#64ffda;border:1px solid #233554;border-radius:6px;text-decoration:none;font-size:13px">"🌐 Remote"</a>
            </div>
            <div style="background:#112240;border:1px solid #233554;border-radius:8px;padding:10px;margin:8px 0">
                <h3 style="color:#64ffda;font-size:13px;margin:0 0 6px">"Session"</h3>
                <p style="color:#8892b0;font-size:12px;margin:0">"platform_run! + WASM UI"</p>
            </div>
            <p style="color:#445566;font-size:10px;margin:12px 0 0">"foundation_platform 0.0.1"</p>
        </div>
    };

    app.mount(dashboard);
    app.stabilize();
    core::mem::forget(app);
}
