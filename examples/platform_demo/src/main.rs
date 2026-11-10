//! Platform Demo — minimal foundation_platform app.

use std::sync::Arc;

use foundation_core::valtron::{Ctx, MonotonicClock};
use foundation_platform::*;

/// The host context. An entropy failure keeps the app running with a
/// deterministic RNG and a warning (issue #426).
fn host_ctx() -> Ctx {
    Ctx::try_host().unwrap_or_else(|error| {
        eprintln!(
            "warning: entropy source unavailable, guest randomness is deterministic: {error}"
        );
        Ctx::fixed(Arc::new(MonotonicClock::new()))
    })
}

fn main() {
    platform_run!(PlatformBuilder::new()
        .ctx(host_ctx())
        .route("/app/*", webview_app().with_profile(Profile::App))
        .route(
            "/remote/*",
            remote_fetch()
                .with_profile(Profile::TrustedRemote)
                .with_cache_policy(CachePolicy::NetworkFirst)
        )
        .route(
            "/cached/*",
            remote_fetch()
                .with_profile(Profile::TrustedRemote)
                .with_cache_policy(CachePolicy::CacheFirst)
        )
        .route("/auth/*", remote_fetch().with_profile(Profile::Auth))
        .setup(|session| {
            println!("=== Platform Demo === Session: {:?}", session.session_id());
        }));
}
