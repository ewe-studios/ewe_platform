//! Capability trait and registry for native platform capabilities.
//!
//! Capabilities are registered at build time and invoked at runtime through
//! the session backbone. Five defense layers: profile gate → registration
//! gate → per-route allowlist → OS permission → stale-page guard.

use std::collections::HashMap;
use std::sync::RwLock;

use foundation_ui_traits::*;

use crate::profiles::{Access, ProfileGate, Service};
use crate::session::PlatformSession;
use crate::types::{CapabilityRequest, CapabilityResponse};

// ── Capability trait ─────────────────────────────────────────────────

/// A native platform capability. Registered with the capability registry
/// and invoked through the session backbone.
///
/// Patterned after `WGPU`/`WASI` — the platform provides the trait,
/// the user implements it, the shell orchestrates.
pub trait Capability: Send + Sync + 'static {
    /// Unique identifier (e.g. "camera", "biometric_auth").
    fn id(&self) -> &CapabilityId;

    /// Minimum WebView profile required to invoke this capability.
    /// The session checks this before execution.
    fn min_profile(&self) -> Profile;

    /// Execute the capability with the given action and payload.
    fn execute(
        &self,
        session: &PlatformSession,
        action: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
}

// ── Capability registry (on PlatformSession) ─────────────────────────

/// Registry of all registered capabilities. Wrapped in RwLock for
/// thread-safe concurrent access (read-heavy: many invocations, few
/// registrations).
pub struct CapabilityRegistry {
    handlers: RwLock<HashMap<String, Box<dyn Capability>>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a capability. Called at startup.
    pub fn register(&self, cap: impl Capability) {
        self.handlers
            .write()
            .unwrap()
            .insert(cap.id().0.clone(), Box::new(cap));
    }

    /// Invoke a capability through the full five-layer defense chain.
    ///
    /// 1. Stale-page guard — is the requesting page still active?
    /// 2. Registration — is the capability registered?
    /// 3. Profile gate — does the route's profile allow native APIs?
    ///    Does the profile meet the capability's minimum?
    /// 4. Per-route allowlist — is the capability in the route's list?
    /// 5. Execute — call the handler
    pub fn invoke(
        &self,
        session: &PlatformSession,
        request: &CapabilityRequest,
        current_route: Option<&RouteDecision>,
    ) -> CapabilityResponse {
        let respond = |status| CapabilityResponse {
            id: request.id.clone(),
            page_identity: request.page_identity.clone(),
            status,
        };

        // Layer 1: Stale-page guard
        if !session.is_active_page(&request.page_identity) {
            return respond(Err("stale page — request from navigated-away page".into()));
        }

        // Hold the read lock for the entire invocation so the handler
        // reference remains valid. Registrations are rare (startup only).
        let guard = self.handlers.read().unwrap();

        // Layer 2: Look up the capability handler
        let handler = match guard.get(&request.capability) {
            Some(h) => h,
            None => return respond(Err(format!("unknown capability: {}", request.capability))),
        };

        // Layer 3: Profile-level gate
        let profile = current_route.map(|r| r.profile).unwrap_or(Profile::UntrustedRemote);
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

        // Layer 5: Execute — guard held until here, then dropped
        let result = handler.execute(session, &request.action, request.payload.clone());
        drop(guard);

        respond(result)
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a profile meets or exceeds a minimum required profile.
/// Trust hierarchy: App > TrustedRemote > UntrustedRemote.
/// Auth and Devtools are special — checked separately by ProfileGate.
fn profile_satisfies(actual: Profile, required: Profile) -> bool {
    fn rank(p: Profile) -> u8 {
        match p {
            Profile::UntrustedRemote => 0,
            Profile::TrustedRemote => 1,
            Profile::App => 2,
            Profile::Auth => 3,    // special — not in trust hierarchy
            Profile::Devtools => 4, // special — not in trust hierarchy
        }
    }
    // Only comparable within the trust hierarchy
    if matches!(actual, Profile::Auth | Profile::Devtools)
        || matches!(required, Profile::Auth | Profile::Devtools)
    {
        // Auth and Devtools don't participate in the trust hierarchy.
        // Auth satisfies Auth only. Devtools satisfies everything (in debug).
        return actual == required
            || (actual == Profile::Devtools && cfg!(debug_assertions));
    }
    rank(actual) >= rank(required)
}

// ── Test helpers ─────────────────────────────────────────────────────

/// Test capability for integration tests.
pub struct TestCap {
    pub id: CapabilityId,
    pub min_profile: Profile,
}

impl Capability for TestCap {
    fn id(&self) -> &CapabilityId { &self.id }
    fn min_profile(&self) -> Profile { self.min_profile }
    fn execute(&self, _: &PlatformSession, action: &str, _: serde_json::Value) -> Result<serde_json::Value, String> {
        Ok(serde_json::Value::String(format!("executed: {action}")))
    }
}

/// Create a test registry with a camera capability registered.
pub fn test_registry() -> CapabilityRegistry {
    let reg = CapabilityRegistry::new();
    reg.register(TestCap {
        id: CapabilityId("camera".into()),
        min_profile: Profile::TrustedRemote,
    });
    reg
}

// Tests moved to tests/capability_suite.rs
