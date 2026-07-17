//! Platform Demo — minimal Tauri app exercising foundation_platform.

use foundation_platform::*;
use tauri::Manager;

fn main() {
    PlatformBuilder::new()
        .register_ewe_protocol()
        .setup(|app| {
            let session = PlatformSession::new();

            session.route("/app/*", webview_app().with_profile(Profile::App));
            session.route("/remote/*", remote_fetch()
                .with_profile(Profile::TrustedRemote)
                .with_cache_policy(CachePolicy::NetworkFirst));
            session.route("/cached/*", remote_fetch()
                .with_profile(Profile::TrustedRemote)
                .with_cache_policy(CachePolicy::CacheFirst));
            session.route("/auth/*", remote_fetch()
                .with_profile(Profile::Auth));

            println!("=== Platform Demo === Session: {:?}", session.session_id());
            app.manage(session);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build Platform Demo")
        .run(|_handle, event| {
            if let tauri::RunEvent::Exit = event {
                println!("Platform Demo shutting down.");
            }
        });
}
