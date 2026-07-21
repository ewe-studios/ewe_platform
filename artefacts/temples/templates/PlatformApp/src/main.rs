//! {{ PROJECT_NAME }} — foundation_platform app.
//!
//! Generated from `cargo platform generate -t PlatformApp -p {{ PROJECT_NAME }}`.

use foundation_platform::*;

fn main() {
    platform_run!(PlatformBuilder::new()
        .route("/app/*", webview_app().with_profile(Profile::App))
        .route("/remote/*", remote_fetch()
            .with_profile(Profile::TrustedRemote)
            .with_cache_policy(CachePolicy::NetworkFirst))
        .setup(|session| {
            // session.capabilities().register(MyCapability);
            println!("{{ PROJECT_NAME }} booted");
        }));
}
