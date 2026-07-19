use foundation_platform::*;

mod generated;

/// Wrap a bare HTML body into a full page with the scheme interceptor, styles, and nav.
fn page_html(title: &str, body: &str, nav: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><title>{title}</title>\
         <style>body{{font-family:sans-serif;padding:16px;background:#0a0a1a;color:#ccd6f6}}h1{{color:#bd93f9;font-size:18px}}a:hover{{background:#233554}}</style>\
         <script>{interceptor}</script></head><body>{body}{nav}</body></html>",
        interceptor = foundation_wasm_ui::embedded::PLATFORM_SCHEME_INTERCEPTOR_JS,
    )
}

fn nav_buttons(current: &str) -> String {
    let pages = [("/app/", "🏠 App"), ("/app-hello/", "👋 Hello"), ("/remote/news", "🌐 Remote"), ("/api/status", "⚡ API")];
    pages.iter()
        .filter(|(p, _)| *p != current)
        .fold(String::new(), |s, (href, label)| {
            s + &format!("<a href=\"ewe://localhost{href}\" style=\"padding:6px 10px;margin:3px;background:#112240;color:#64ffda;border:1px solid #233554;border-radius:5px;text-decoration:none;font-size:12px;display:inline-block\">{label}</a>")
        })
}

fn setup_routes(session: &PlatformSession) {
    let app_responder = generated::app::AppAssets::build();
    let hello_responder = generated::app_hello::AppAssets::build();

    session.register_route_with(
        "/app/*",
        webview_app().with_profile(Profile::App),
        app_responder,
    );
    session.register_route_with(
        "/app-hello/*",
        webview_app().with_profile(Profile::App),
        hello_responder,
    );

    struct RemoteFetch { base_url: String }
    impl RouteResponder for RemoteFetch {
        fn respond(&self, intent: &NavigationIntent, _: &RouteDecision, _: &PlatformSession) -> tauri::http::Response<Vec<u8>> {
            let route = pattern::extract_path(&intent.url);
            let body = format!("<h1>{route}</h1><p>Remote fetch from {base}</p>", base = &self.base_url);
            let html = page_html(&route, &body, &nav_buttons("/remote/"));
            tauri::http::Response::builder().status(200).header("Content-Type", "text/html; charset=utf-8").body(html.into_bytes()).unwrap()
        }
    }
    session.register_route_with("/remote/*", remote_fetch().with_profile(Profile::TrustedRemote), RemoteFetch { base_url: "https://api.example.com".into() });

    struct IpcEmit;
    impl RouteResponder for IpcEmit {
        fn respond(&self, intent: &NavigationIntent, _: &RouteDecision, _: &PlatformSession) -> tauri::http::Response<Vec<u8>> {
            let route = pattern::extract_path(&intent.url);
            let body = format!("<h1>{route}</h1><p>IPC Shell dispatch</p>");
            let html = page_html(&route, &body, &nav_buttons("/api/"));
            tauri::http::Response::builder().status(200).header("Content-Type", "text/html; charset=utf-8").body(html.into_bytes()).unwrap()
        }
    }
    session.register_route_with("/api/*", ipc_shell_with("shell"), IpcEmit);

    println!("[platform_android] Session: {:?}", session.session_id());
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    platform_run!(PlatformBuilder::new()
        .setup(setup_routes));
}
