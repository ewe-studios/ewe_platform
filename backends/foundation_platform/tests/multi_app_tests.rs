//! App isolation tests moved out of `src/multi_app.rs` (spec 70 feature 59,
//! rule 7). Every item the tests use is public.

use foundation_platform::{AppConfig, AppIsolation, CachePolicy, Profile};

#[test]
fn app_isolation_longest_prefix_match() {
    let isolation = AppIsolation::new();
    isolation.register(AppConfig::new("/app/", "app"));
    isolation.register(AppConfig::new("/app/settings/", "app-settings"));

    assert_eq!(isolation.webview_label_for("/app/home"), "app");
    assert_eq!(
        isolation.webview_label_for("/app/settings/profile"),
        "app-settings"
    );
    assert_eq!(isolation.webview_label_for("/api/status"), "main");
}

#[test]
fn app_isolation_capability_check() {
    let isolation = AppIsolation::new();
    isolation.register(AppConfig::new("/app/", "app").with_capabilities(["camera", "storage"]));

    assert!(isolation.is_capability_allowed("/app/home", "camera"));
    assert!(!isolation.is_capability_allowed("/app/home", "bluetooth"));
    assert!(!isolation.is_capability_allowed("/api/status", "camera"));
}

#[test]
fn app_config_builders() {
    let config = AppConfig::new("/app/", "app")
        .with_profile(Profile::TrustedRemote)
        .with_cache(CachePolicy::NetworkFirst)
        .with_capabilities(["camera", "location"])
        .with_remote_fetch(true)
        .with_ipc(true);

    assert_eq!(config.webview_label, "app");
    assert_eq!(config.profile, Profile::TrustedRemote);
    assert_eq!(config.cache_policy, CachePolicy::NetworkFirst);
    assert!(config.allow_remote_fetch);
    assert!(config.allow_ipc);
    assert_eq!(config.allowed_capabilities.len(), 2);
}
