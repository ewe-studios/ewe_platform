//! Platform Android — WASM UI validation dashboard.
//!
//! Exercises every presentation mode and IPC bridge from WASM through
//! foundation_wasm_ui and foundation_platform_native.
//!
//! ## IPC paths
//!
//! Two paths exist for WASM→host IPC:
//!
//! 1. **JS bridge** (working): `window.invokeIpc("chrome", "present_modal", {})`
//!    → `__TAURI_INTERNALS__.invoke('__ewe_ipc', ...)` → async → native handler
//!
//! 2. **WASM import** (Tauri gap): `ipc_dispatch("chrome", req)` →
//!    `host_ipc_invoke(ptr, len)` → synchronous WASM import → needs sync
//!    handler ← NOT wired for Tauri yet (Tauri's invoke is async)
//!
//! Path 1 is proven by the IPC demo pages. Path 2 is the goal — WASM apps
//! use typed wrappers (`Modal::present(PresentArgs{...})`) which call
//! `ipc_dispatch`. For now, the WASM wrappers exist and compile but the
//! `host_ipc_invoke` import is not bridged to Tauri's async invoke.
//!
//! The typed WASM wrappers in `foundation_platform_native::wasm::modal`
//! (and `dialog`) ARE the correct API surface — they compile on wasm32,
//! import into any WASM app, and their `WirePayload` impls ensure
//! consistent serialization between WASM and native. The gap is the
//! transport layer under `ipc_dispatch`, not the API design.

use foundation_macros::wasm_bin;
use foundation_wasm_ui::html;
use foundation_wasm_ui::App;
use foundation_wasm_ui::install_event_bridge;

use foundation_platform_native::shared::modal_types::{PresentArgs, DismissArgs};
use foundation_platform_native::wasm::modal::Modal;

use foundation_platform_native::shared::dialog_types::ShowArgs;
use foundation_platform_native::wasm::dialog::Dialog;

#[wasm_bin(extern = "true")]
fn platform_dashboard() {
    let app = App::new();
    install_event_bridge(app.signals().clone());
    let (ctx, rcv) = app.context();

    let on_present_modal = ctx.callback(move |_event| {
        let _ = Modal::present(
            PresentArgs {
                route: "http://ewe.localhost/app/".into(),
                style: Some("bottom_sheet".into()),
                title: Some("Native Modal".into()),
            },
            |_r| {},
        );
    });

    let on_dismiss_modal = ctx.callback(move |_event| {
        let _ = Modal::dismiss(
            DismissArgs { modal_id: String::new() },
            |_r| {},
        );
    });

    let on_show_dialog = ctx.callback(move |_event| {
        let _ = Dialog::show(
            ShowArgs {
                title: "WASM Dialog".into(),
                message: Some("This dialog was triggered from WASM via IPC".into()),
                positive_button: Some("OK".into()),
                negative_button: Some("Cancel".into()),
                route: None,
            },
            |_r| {},
        );
    });

    let dashboard = html! { ctx, rcv,
        <div style="padding:16px;font-family:sans-serif;background:#0a0a1a;color:#ccd6f6;min-height:100vh">
            <h1 style="color:#64ffda;font-size:22px;margin:0 0 4px">"Foundation Platform"</h1>
            <p style="color:#8892b0;font-size:12px;margin:0 0 16px">"Android — WASM UI · F42 Validation"</p>

            <div style="display:flex;gap:6px;flex-wrap:wrap;margin:0 0 12px">
                <a href="ewe://localhost/app/"
                   style="padding:8px 14px;background:#112240;color:#64ffda;border:1px solid #233554;border-radius:6px;text-decoration:none;font-size:13px">"🏠 App"</a>
                <a href="ewe://localhost/app-hello/"
                   style="padding:8px 14px;background:#112240;color:#64ffda;border:1px solid #233554;border-radius:6px;text-decoration:none;font-size:13px">"👋 Hello"</a>
                <a href="ewe://localhost/api/invoke"
                   style="padding:8px 14px;background:#112240;color:#64ffda;border:1px solid #233554;border-radius:6px;text-decoration:none;font-size:13px">"⚡ IPC"</a>
                <a href="ewe://localhost/remote/example.com"
                   style="padding:8px 14px;background:#112240;color:#64ffda;border:1px solid #233554;border-radius:6px;text-decoration:none;font-size:13px">"🌐 Remote"</a>
            </div>

            <div style="background:#112240;border:1px solid #233554;border-radius:8px;padding:10px;margin:8px 0">
                <h3 style="color:#ffb86c;font-size:14px;margin:0 0 8px">"🎬 Presentation Modes"</h3>
                <p style="color:#8892b0;font-size:11px;margin:0 0 8px">"Route-based: handler sets Presentation on RouteDecision"</p>
                <div style="display:flex;gap:6px;flex-wrap:wrap">
                    <a href="ewe://localhost/nav_push"
                       style="padding:6px 12px;background:#1a3a2a;color:#64ffda;border:1px solid #2a5a3a;border-radius:4px;text-decoration:none;font-size:12px">"📤 Push"</a>
                    <a href="ewe://localhost/nav_modal"
                       style="padding:6px 12px;background:#1a3a2a;color:#64ffda;border:1px solid #2a5a3a;border-radius:4px;text-decoration:none;font-size:12px">"📋 Modal"</a>
                    <a href="ewe://localhost/nav_morph"
                       style="padding:6px 12px;background:#1a3a2a;color:#64ffda;border:1px solid #2a5a3a;border-radius:4px;text-decoration:none;font-size:12px">"🔄 Morph"</a>
                    <a href="ewe://localhost/nav_replace"
                       style="padding:6px 12px;background:#1a3a2a;color:#64ffda;border:1px solid #2a5a3a;border-radius:4px;text-decoration:none;font-size:12px">"🔀 Replace"</a>
                </div>
            </div>

            <div style="background:#112240;border:1px solid #233554;border-radius:8px;padding:10px;margin:8px 0">
                <h3 style="color:#ffb86c;font-size:14px;margin:0 0 8px">"📱 Native Capabilities (WASM → IPC)"</h3>
                <p style="color:#8892b0;font-size:11px;margin:0 0 8px">"Typed WASM wrappers compiled in. Buttons wired via EventDispatcher."</p>
                <div style="display:flex;gap:6px;flex-wrap:wrap">
                    <button primal:onclick={on_present_modal}
                       style="padding:8px 14px;background:#3a2a1a;color:#ffb86c;border:1px solid #5a3a2a;border-radius:6px;font-size:13px;cursor:pointer">"⬇️ Present Modal (bottom_sheet)"</button>
                    <button primal:onclick={on_dismiss_modal}
                       style="padding:8px 14px;background:#3a2a1a;color:#ffb86c;border:1px solid #5a3a2a;border-radius:6px;font-size:13px;cursor:pointer">"⬆️ Dismiss Modal"</button>
                    <button primal:onclick={on_show_dialog}
                       style="padding:8px 14px;background:#3a2a1a;color:#ffb86c;border:1px solid #5a3a2a;border-radius:6px;font-size:13px;cursor:pointer">"💬 Show Dialog"</button>
                </div>
            </div>

            <div style="background:#112240;border:1px solid #233554;border-radius:8px;padding:10px;margin:8px 0">
                <h3 style="color:#64ffda;font-size:14px;margin:0 0 8px">"⚡ IPC Debug Pages"</h3>
                <div style="display:flex;gap:6px;flex-wrap:wrap">
                    <a href="ewe://localhost/api/system"
                       style="padding:6px 12px;background:#1a3a2a;color:#64ffda;border:1px solid #2a5a3a;border-radius:4px;text-decoration:none;font-size:12px">"💻 System"</a>
                    <a href="ewe://localhost/api/events"
                       style="padding:6px 12px;background:#1a3a2a;color:#64ffda;border:1px solid #2a5a3a;border-radius:4px;text-decoration:none;font-size:12px">"📡 Events"</a>
                    <a href="ewe://localhost/api/capability"
                       style="padding:6px 12px;background:#1a3a2a;color:#64ffda;border:1px solid #2a5a3a;border-radius:4px;text-decoration:none;font-size:12px">"🔐 Capabilities"</a>
                    <a href="ewe://localhost/presentation/"
                       style="padding:6px 12px;background:#1a3a2a;color:#64ffda;border:1px solid #2a5a3a;border-radius:4px;text-decoration:none;font-size:12px">"🎬 Presentation"</a>
                </div>
            </div>

            <p style="color:#445566;font-size:10px;margin:12px 0 0">"foundation_platform 0.0.1 · foundation_platform_native 0.0.1"</p>
        </div>
    };

    app.mount(dashboard);
    app.stabilize();
    core::mem::forget(app);
}
