//! Per-app WebView isolation (F29 Stage 6).
//!
//! WHY: When multiple `wasm_app`s run in the same process, each needs its
//! own WebView with its own `Profile`, `CachePolicy`, and capability
//! allowlist. Without isolation, one app could access another app's
//! capabilities or cached data.
//!
//! WHAT: `AppConfig` maps a route prefix to a WebView label, `Profile`,
//! `CachePolicy`, and capability allowlist. `AppIsolation` holds the
//! registry of app configs and resolves them for each navigation.
//!
//! HOW: During route resolution, `AppIsolation::resolve()` returns the
//! target WebView label and capability allowlist. The session's
//! `execute_decision()` uses this to route navigation to the correct
//! WebView and enforce per-app capability gates.

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use foundation_ui_traits::{CachePolicy, Profile};

// ── App config ──────────────────────────────────────────────────────────

/// Configuration for a single app's WebView isolation.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// The route prefix this app handles (e.g. "/app/", "/app-hello/").
    pub route_prefix: String,
    /// The WebView label used for this app's WebView.
    pub webview_label: String,
    /// Minimum profile for this app's WebView.
    pub profile: Profile,
    /// Cache policy for this app's routes.
    pub cache_policy: CachePolicy,
    /// Capabilities this app is allowed to invoke.
    pub allowed_capabilities: HashSet<String>,
    /// Whether this app can access the HTTP backend.
    pub allow_remote_fetch: bool,
    /// Whether this app can invoke IPC handlers.
    pub allow_ipc: bool,
}

impl AppConfig {
    /// Create a new app config with sensible defaults.
    #[must_use]
    pub fn new(route_prefix: &str, webview_label: &str) -> Self {
        Self {
            route_prefix: route_prefix.to_string(),
            webview_label: webview_label.to_string(),
            profile: Profile::App,
            cache_policy: CachePolicy::CacheFirst,
            allowed_capabilities: HashSet::new(),
            allow_remote_fetch: false,
            allow_ipc: false,
        }
    }

    /// Builder: set the profile.
    #[must_use]
    pub fn with_profile(mut self, profile: Profile) -> Self {
        self.profile = profile;
        self
    }

    /// Builder: set the cache policy.
    #[must_use]
    pub fn with_cache(mut self, policy: CachePolicy) -> Self {
        self.cache_policy = policy;
        self
    }

    /// Builder: allow specific capabilities.
    #[must_use]
    pub fn with_capabilities(mut self, caps: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.allowed_capabilities = caps.into_iter().map(Into::into).collect();
        self
    }

    /// Builder: allow remote fetch.
    #[must_use]
    pub fn with_remote_fetch(mut self, allow: bool) -> Self {
        self.allow_remote_fetch = allow;
        self
    }

    /// Builder: allow IPC dispatch.
    #[must_use]
    pub fn with_ipc(mut self, allow: bool) -> Self {
        self.allow_ipc = allow;
        self
    }
}

// ── App isolation ───────────────────────────────────────────────────────

/// Registry of per-app isolation configs. Thread-safe via `RwLock`.
///
/// At startup, each `wasm_app` registers its config. On each navigation,
/// `resolve()` returns the matching app's config, or `None` for routes
/// that don't match any app (platform-level routes like `/api/*`).
pub struct AppIsolation {
    apps: RwLock<HashMap<String, AppConfig>>,
}

impl AppIsolation {
    #[must_use]
    pub fn new() -> Self {
        Self {
            apps: RwLock::new(HashMap::new()),
        }
    }

    /// Register an app config. The `route_prefix` is the key.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn register(&self, config: AppConfig) {
        self.apps
            .write()
            .unwrap()
            .insert(config.route_prefix.clone(), config);
    }

    /// Resolve the app config for a given route. Returns the longest-prefix
    /// match (most specific app wins).
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn resolve(&self, route: &str) -> Option<AppConfig> {
        let apps = self.apps.read().unwrap();
        apps.iter()
            .filter(|(prefix, _)| route.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, config)| config.clone())
    }

    /// Get the WebView label for a route. Returns `"main"` if no app matches.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn webview_label_for(&self, route: &str) -> String {
        self.resolve(route)
            .map(|c| c.webview_label)
            .unwrap_or_else(|| "main".to_string())
    }

    /// Check if a capability is allowed for the given route.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn is_capability_allowed(&self, route: &str, capability: &str) -> bool {
        self.resolve(route)
            .map(|c| c.allowed_capabilities.contains(capability))
            .unwrap_or(false)
    }

    /// Return all registered app prefixes.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn prefixes(&self) -> Vec<String> {
        self.apps.read().unwrap().keys().cloned().collect()
    }
}

impl Default for AppIsolation {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
        isolation.register(
            AppConfig::new("/app/", "app").with_capabilities(["camera", "storage"]),
        );

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
}
