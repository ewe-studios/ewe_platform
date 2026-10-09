use foundation_core::valtron::valtron;
use foundation_platform::*;
use foundation_ui_traits::Profile;

mod generated;

fn setup_routes(session: &PlatformSession) {
    let root = session.bundle_root();
    let app_responder = generated::app::AppAssets::build(root);

    session.register_route_with(
        "/app/*",
        webview_app().with_profile(Profile::App),
        app_responder,
    );

    // Remote proxy — platform RemoteProxy fetches real URLs via session.http_backend().
    session.register_route_with(
        "/remote/*",
        remote_fetch().with_profile(Profile::TrustedRemote),
        RemoteProxy::new(),
    );

    println!("[platform_ios] Session: {:?}", session.session_id());
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[valtron]
pub fn run() {
    platform_run!(PlatformBuilder::new()
        .inject_platform_runtimes()
        .setup(setup_routes));
}
