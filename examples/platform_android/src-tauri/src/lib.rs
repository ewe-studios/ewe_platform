use foundation_platform::*;

#[path = "generated/mod.rs"]
mod generated;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ── WASM app: serves public/app/ via EmbedDirectoryAs ──
    let app_responder = generated::app::AppAssets::build();

    // ── Second WASM app: serves public/app-hello/ ──
    let hello_responder = generated::app_hello::AppAssets::build();

    // ── Remote server: example responder that fetches from a URL ──
    struct RemoteFetch {
        base_url: String,
    }
    impl RouteResponder for RemoteFetch {
        fn respond(
            &self,
            intent: &NavigationIntent,
            _decision: &RouteDecision,
            _session: &PlatformSession,
        ) -> tauri::http::Response<Vec<u8>> {
            let route = foundation_platform::pattern::extract_path(&intent.url);
            // In a real app this would do an HTTP fetch. MVP: return a page
            // showing what would have been fetched.
            let html = format!(
                "<!DOCTYPE html><html><head><meta charset=utf-8><title>{route}</title>\
                 <style>body{{font-family:sans-serif;padding:16px;background:#0a1a2a;color:#ccd6f6}}\
                 h1{{color:#bd93f9;font-size:18px}}</style></head>\
                 <body><h1>{route}</h1><p>Remote fetch from {base}</p></body></html>",
                base = &self.base_url,
            );
            tauri::http::Response::builder()
                .status(200)
                .header("Content-Type", "text/html; charset=utf-8")
                .body(html.into_bytes())
                .unwrap()
        }
    }

    // ── IPC shell: example responder that emits a Tauri event ──
    struct IpcEmit;
    impl RouteResponder for IpcEmit {
        fn respond(
            &self,
            intent: &NavigationIntent,
            _decision: &RouteDecision,
            _session: &PlatformSession,
        ) -> tauri::http::Response<Vec<u8>> {
            let route = foundation_platform::pattern::extract_path(&intent.url);
            let html = format!(
                "<!DOCTYPE html><html><head><meta charset=utf-8><title>{route}</title>\
                 <style>body{{font-family:sans-serif;padding:16px;background:#0a1a2a;color:#ccd6f6}}\
                 h1{{color:#ffb86c;font-size:18px}}</style></head>\
                 <body><h1>{route}</h1><p>IPC Shell dispatch</p></body></html>"
            );
            tauri::http::Response::builder()
                .status(200)
                .header("Content-Type", "text/html; charset=utf-8")
                .body(html.into_bytes())
                .unwrap()
        }
    }

    platform_run!(PlatformBuilder::new()
        // ── WASM app: embedded assets from public/app/ ──
        .route_with(
            "/app/*",
            webview_app().with_profile(Profile::App),
            app_responder
        )
        // ── Second WASM app: embedded assets from public/app-hello/ ──
        .route_with(
            "/app-hello/*",
            webview_app().with_profile(Profile::App),
            hello_responder
        )
        // ── Remote server: fetches from a backend URL ──
        .route_with(
            "/remote/*",
            remote_fetch().with_profile(Profile::TrustedRemote),
            RemoteFetch {
                base_url: "https://api.example.com".into()
            }
        )
        // ── IPC shell: dispatches to native shell process ──
        .route_with("/api/*", ipc_shell_with("shell"), IpcEmit)
        .setup(|session| {
            println!("[platform_android] Session: {:?}", session.session_id());
        }));
}
