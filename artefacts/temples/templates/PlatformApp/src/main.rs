//! {{ PROJECT_NAME }} — foundation_platform Tauri app.
//!
//! Generated from `cargo platform generate -t PlatformApp -p {{ PROJECT_NAME }}`.
//! Boots a PlatformSession, registers routes, and runs.

use foundation_platform::*;

fn main() {
    PlatformBuilder::new()
        .route("/app/*", webview_app().with_profile(Profile::App))
        .route("/remote/*", remote_fetch()
            .with_profile(Profile::TrustedRemote)
            .with_cache_policy(CachePolicy::NetworkFirst))
        .setup(|session| {
            // session.capabilities().register(MyCapability);
            println!("{{ PROJECT_NAME }} booted — session {:?}", session.session_id());
        })
        .build(tauri::generate_context!())
        .expect("failed to build {{ PROJECT_NAME }}")
        .run(|_handle, event| {
            if let tauri::RunEvent::Exit = event {
                println!("{{ PROJECT_NAME }} shutting down.");
            }
        });
}
