//! Platform Demo — minimal foundation_platform app.

use foundation_platform::*;

fn main() {
    platform_run!(PlatformBuilder::new()
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
        }));
}
