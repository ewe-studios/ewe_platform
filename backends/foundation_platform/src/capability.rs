//! Capability trait and registry for native platform capabilities.
//!
//! Capabilities are registered at build time and invoked at runtime through
//! the session backbone. Five defense layers: profile gate → registration
//! gate → per-route allowlist → OS permission → stale-page guard.
//!
//! NOTE: These are the F05 native `serde_json::Value`-based types. F23 moved
//! portable capability primitives to `foundation_wasm::capability`. The F05
//! types are renamed with the `Native` prefix to avoid collision.

use std::collections::HashMap;
use std::sync::RwLock;

use foundation_ui_traits::{CapabilityId, Profile, RouteDecision};

use crate::profiles::{Access, ProfileGate, Service};
use crate::session::PlatformSession;
use crate::types::{NativeCapabilityRequest, NativeCapabilityResponse};

// ── NativeCapability trait ─────────────────────────────────────────────

/// A native platform capability (F05). Registered with the capability registry
/// and invoked through the session backbone with the 5-layer defense.
///
/// Patterned after `WGPU`/`WASI` — the platform provides the trait,
/// the user implements it, the shell orchestrates.
///
/// For portable (wasm32+native) capabilities, use
/// `foundation_wasm::WasmCapability` (F23).
pub trait NativeCapability: Send + Sync + 'static {
    /// Unique identifier (e.g. "camera", "`biometric_auth`").
    fn id(&self) -> &CapabilityId;

    /// Minimum `WebView` profile required to invoke this capability.
    fn min_profile(&self) -> Profile;

    /// Execute the capability with the given action and payload.
    ///
    /// # Errors
    ///
    /// Returns an error string if the capability execution fails.
    fn execute(
        &self,
        session: &PlatformSession,
        action: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
}

// ── Capability registry (on PlatformSession) ───────────────────────────

/// Registry of all registered native capabilities (F05). Wrapped in `RwLock`
/// for thread-safe concurrent access.
pub struct NativeCapabilityRegistry {
    handlers: RwLock<HashMap<String, Box<dyn NativeCapability>>>,
}

impl NativeCapabilityRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a native capability. Called at startup.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn register(&self, cap: impl NativeCapability) {
        self.handlers
            .write()
            .unwrap()
            .insert(cap.id().0.clone(), Box::new(cap));
    }

    /// Invoke a capability through the full five-layer defense chain.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn invoke(
        &self,
        session: &PlatformSession,
        request: &NativeCapabilityRequest,
        current_route: Option<&RouteDecision>,
    ) -> NativeCapabilityResponse {
        let respond = |status| NativeCapabilityResponse {
            id: request.id.clone(),
            page_identity: request.page_identity.clone(),
            status,
        };

        // Layer 1: Stale-page guard
        if !session.is_active_page(&request.page_identity) {
            return respond(Err("stale page — request from navigated-away page".into()));
        }

        let guard = self.handlers.read().unwrap();

        // Layer 2: Look up the capability handler
        let Some(handler) = guard.get(&request.capability) else {
            return respond(Err(format!("unknown capability: {}", request.capability)));
        };

        // Layer 3: Profile-level gate
        let profile = current_route
            .map_or(Profile::UntrustedRemote, |r| r.profile);
        let gate = ProfileGate::new(profile);
        if let Err(e) = gate.check(Service::NativeApi, Access::Execute) {
            return respond(Err(e.to_string()));
        }
        if !profile_satisfies(profile, handler.min_profile()) {
            return respond(Err(format!(
                "profile {:?} too low for capability '{}' (requires {:?})",
                profile,
                handler.id().0,
                handler.min_profile()
            )));
        }

        // Layer 4: Per-route allowlist gate
        if let Some(route) = current_route {
            if !route.capabilities.is_empty()
                && !route.capabilities.iter().any(|c| c == handler.id())
            {
                return respond(Err(format!(
                    "capability '{}' not allowed on this route",
                    handler.id().0
                )));
            }
        }

        // Layer 5: Execute
        let result = handler.execute(session, &request.action, request.payload.clone());
        drop(guard);

        respond(result)
    }
}

impl Default for NativeCapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a profile meets or exceeds a minimum required profile.
fn profile_satisfies(actual: Profile, required: Profile) -> bool {
    fn rank(p: Profile) -> u8 {
        match p {
            Profile::UntrustedRemote => 0,
            Profile::TrustedRemote => 1,
            Profile::App => 2,
            Profile::Auth => 3,
            Profile::Devtools => 4,
        }
    }
    if matches!(actual, Profile::Auth | Profile::Devtools)
        || matches!(required, Profile::Auth | Profile::Devtools)
    {
        return actual == required
            || (actual == Profile::Devtools && cfg!(debug_assertions));
    }
    rank(actual) >= rank(required)
}

// ── Test helpers ───────────────────────────────────────────────────────

/// Test capability for integration tests.
pub struct TestNativeCap {
    pub id: CapabilityId,
    pub min_profile: Profile,
}

impl NativeCapability for TestNativeCap {
    fn id(&self) -> &CapabilityId { &self.id }
    fn min_profile(&self) -> Profile { self.min_profile }
    fn execute(&self, _: &PlatformSession, action: &str, _: serde_json::Value) -> Result<serde_json::Value, String> {
        Ok(serde_json::Value::String(format!("executed: {action}")))
    }
}

/// Create a test registry with a camera capability registered.
#[must_use]
pub fn test_native_registry() -> NativeCapabilityRegistry {
    let reg = NativeCapabilityRegistry::new();
    reg.register(TestNativeCap {
        id: CapabilityId("camera".into()),
        min_profile: Profile::TrustedRemote,
    });
    reg
}
