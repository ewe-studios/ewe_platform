use foundation_platform::*;

/// Generated per-app modules. Each app crate under `../app-{name}/`
/// produces a module here — `generate_platform_code()` auto-discovers
/// `#[wasm_bin]` entrypoints and creates `WebviewApp<AppAssets>` wrappers.
#[path = "generated/mod.rs"]
mod generated;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    platform_run!(PlatformBuilder::new()
        // ── WASM app: served from `public/app/` via generated module ──
        .route_with("/app/*",
            webview_app().with_profile(Profile::App),
            generated::app::AppAssets::build())

        // ── Remote server: user implements RouteResponder ───────────
        // .route_with("/remote/*",
        //     remote_fetch().with_profile(Profile::TrustedRemote),
        //     MyRemoteServer::new("https://api.example.com"))

        // ── IPC shell: user implements RouteResponder ───────────────
        // .route_with("/api/*",
        //     ipc_shell_with("shell"),
        //     MyIpcShell::new(handle))

        .setup(|session| {
            println!("[platform_android] Session: {:?}", session.session_id());
        }));
}
