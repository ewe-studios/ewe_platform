//! Platform Demo — minimal foundation_platform app.
//!
//! Proves: PlatformBuilder boots, session created, routes registered,
//! ewe:// protocol handler intercepts navigations.

use foundation_platform::*;

fn main() {
    PlatformBuilder::new()
        .route("/app/*", webview_app().with_profile(Profile::App))
        .route("/remote/*", remote_fetch()
            .with_profile(Profile::TrustedRemote)
            .with_cache_policy(CachePolicy::NetworkFirst))
        .route("/cached/*", remote_fetch()
            .with_profile(Profile::TrustedRemote)
            .with_cache_policy(CachePolicy::CacheFirst))
        .route("/auth/*", remote_fetch()
            .with_profile(Profile::Auth))
        .setup(|session| {
            println!("=== Platform Demo === Session: {:?}", session.session_id());
        })
        .build(tauri::generate_context!())
        .expect("failed to build Platform Demo")
        .run(|_handle, event| {
            if let tauri::RunEvent::Exit = event {
                println!("Platform Demo shutting down.");
            }
        });
}
