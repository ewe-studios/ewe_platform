use foundation_platform::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    platform_run!(PlatformBuilder::new()
        .route("/app/*", webview_app().with_profile(Profile::App))
        .route("/remote/*", remote_fetch()
            .with_profile(Profile::TrustedRemote)
            .with_cache_policy(CachePolicy::NetworkFirst))
        .route("/cached/*", remote_fetch()
            .with_cache_policy(CachePolicy::CacheFirst))
        .route("/auth/*", remote_fetch()
            .with_profile(Profile::Auth))
        .route("/api/*", ipc_shell_with("shell"))
        .setup(|session| {
            println!("[platform_android] Session: {:?}", session.session_id());
        }));
}
