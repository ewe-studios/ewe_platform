use foundation_platform::*;

fn main() {
    platform_run!(PlatformBuilder::new()
        .route("/app/*", webview_app().with_profile(Profile::App))
        .route("/remote/*", remote_fetch()
            .with_profile(Profile::TrustedRemote)
            .with_cache_policy(CachePolicy::NetworkFirst))
        .route("/cached/*", remote_fetch()
            .with_cache_policy(CachePolicy::CacheFirst))
        .setup(|session| {
            println!("Android Platform — session {:?}", session.session_id());
        }));
}
