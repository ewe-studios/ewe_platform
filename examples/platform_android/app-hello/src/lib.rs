//! App Hello — second WASM app showcasing multi-app support.

use foundation_macros::wasm_bin;
use foundation_wasm_ui::html;
use foundation_wasm_ui::App;

#[wasm_bin(extern = "true")]
fn hello_dashboard() {
    let app = App::new();
    let (ctx, rcv) = app.context();

    let page = html! { ctx, rcv,
        <div style="padding:16px;font-family:sans-serif;background:#0a0a1a;color:#ccd6f6">
            <h1 style="color:#64ffda;font-size:22px;margin:0 0 4px">"Hello App"</h1>
            <p style="color:#8892b0;font-size:12px;margin:0 0 16px">"Second WASM UI app — multi-app works"</p>
            <div style="background:#112240;border:1px solid #233554;border-radius:8px;padding:10px;margin:8px 0">
                <h3 style="color:#64ffda;font-size:13px;margin:0 0 6px">"Info"</h3>
                <p style="color:#8892b0;font-size:12px;margin:0">"app-hello crate in public/app-hello/"</p>
            </div>
            <p style="color:#445566;font-size:10px;margin:12px 0 0">"foundation_platform 0.0.1"</p>
        </div>
    };

    app.mount(page);
    app.stabilize();
    core::mem::forget(app);
}
