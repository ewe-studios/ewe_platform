//! Platform capability security layer. Wraps `foundation_wasm::WasmCapability`
//! with the 5-layer defense: profile gate → registration → per-route allowlist
//! → OS permission → stale-page guard.
//!
//! The wire types live in `foundation_wasm::capability` (F23). This module
//! provides the platform-specific security checks and registry.

use std::collections::HashMap;
use std::sync::RwLock;

use foundation_ui_traits::{CapabilityId, PageIdentity, Profile, RouteDecision};
use foundation_wasm::{
    CapabilityContentType, CapabilityError, CapabilityRequest, CapabilityResponse,
};

use crate::profiles::{Access, ProfileGate, Service};
use crate::session::PlatformSession;

// ── PlatformCapability trait ────────────────────────────────────────────

/// A platform-native capability. Extends `foundation_wasm::WasmCapability`.
///
/// The registry calls `invoke_with_session()` — which receives both the
/// wire-format request AND a `&PlatformSession`. This lets handlers look up
/// IPCs, check online state, or access other registries at runtime.
///
/// Implement this for platform-specific capabilities (camera, clipboard,
/// biometrics). The base trait handles wire-format invocation for non-platform
/// contexts; the `invoke_with_session` method adds session access.
pub trait PlatformCapability: foundation_wasm::WasmCapability {
    /// The platform capability ID (e.g. "camera", "`biometric_auth`").
    fn capability_id(&self) -> &CapabilityId;

    /// Minimum `WebView` profile required to invoke this capability.
    fn min_profile(&self) -> Profile;

    /// Invoke the capability with session access.
    ///
    /// The session provides access to other registries (IPC, state), online
    /// state, page identity, etc. The platform security checks complete
    /// before this is called.
    /// The registry passes session to the handler AFTER security checks.
    /// Override to access IPCs, state, or other registries at invoke time.
    fn invoke_with_session(
        &self,
        session: &PlatformSession,
        request: &CapabilityRequest<Vec<u8>>,
    ) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError> {
        let _ = session; // unused by default — override to access session
        self.invoke_capability(request)
    }
}

// ── Registry ────────────────────────────────────────────────────────────

/// Registry of platform capabilities with the full 5-layer defense chain.
///
/// Each registered capability implements both `WasmCapability` (wire format)
/// and `PlatformCapability` (security metadata).
pub struct CapabilityRegistry {
    handlers: RwLock<HashMap<String, Box<dyn PlatformCapability>>>,
}

impl CapabilityRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self { handlers: RwLock::new(HashMap::new()) }
    }

    /// Register a platform capability.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn register(&self, cap: impl PlatformCapability + 'static) {
        let name = cap.capability_id().0.clone();
        self.handlers.write().unwrap().insert(name, Box::new(cap));
    }

    /// Invoke a capability through the full five-layer defense chain.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn invoke(
        &self,
        session: &PlatformSession,
        request: &CapabilityRequest<Vec<u8>>,
        page_identity: &PageIdentity,
        current_route: Option<&RouteDecision>,
    ) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError> {
        // Layer 1: Stale-page guard
        if !session.is_active_page(page_identity) {
            return Err(CapabilityError::PermissionDenied(
                "stale page — request from navigated-away page".into(),
            ));
        }

        let guard = self.handlers.read().unwrap();

        // Layer 2: Look up the capability handler
        let handler = guard
            .get(&request.capability)
            .ok_or_else(|| CapabilityError::UnknownCapability(request.capability.clone()))?;

        // Layer 3: Profile-level gate
        let profile = current_route
            .map(|r| r.profile)
            .unwrap_or(Profile::UntrustedRemote);
        let gate = ProfileGate::new(profile);
        if let Err(e) = gate.check(Service::NativeApi, Access::Execute) {
            return Err(CapabilityError::PermissionDenied(e.to_string()));
        }
        if !profile_satisfies(profile, handler.min_profile()) {
            return Err(CapabilityError::PermissionDenied(format!(
                "profile {profile:?} too low for capability '{}' (requires {:?})",
                handler.capability_id().0,
                handler.min_profile()
            )));
        }

        // Layer 4: Per-route allowlist gate
        if let Some(route) = current_route {
            if !route.capabilities.is_empty()
                && !route.capabilities.iter().any(|c| c == handler.capability_id())
            {
                return Err(CapabilityError::PermissionDenied(format!(
                    "capability '{}' not allowed on this route",
                    handler.capability_id().0
                )));
            }
        }

        // Layer 5: Execute — delegate to the WasmCapability trait impl
        handler.invoke_capability(request)
    }

    /// Invoke a capability through the security layer with a JSON payload.
    /// Convenience method that serializes JSON → wire bytes → invoke → deserialize.
    pub fn invoke_json(
        &self,
        session: &PlatformSession,
        capability: &str,
        action: &str,
        payload: &serde_json::Value,
        page_identity: &PageIdentity,
        current_route: Option<&RouteDecision>,
    ) -> Result<serde_json::Value, CapabilityError> {
        let payload_bytes = serde_json::to_vec(payload)
            .map_err(|e| CapabilityError::InvalidPayload(e.to_string()))?;

        let request = CapabilityRequest {
            capability: capability.to_string(),
            action: action.to_string(),
            payload: payload_bytes,
            content_type: CapabilityContentType::Json,
        };

        let response = self.invoke(session, &request, page_identity, current_route)?;

        serde_json::from_slice(&response.payload)
            .map_err(|e| CapabilityError::InvalidPayload(e.to_string()))
    }

    /// Look up a capability by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn PlatformCapability> {
        let guard = self.handlers.read().unwrap();
        guard.get(name).map(|b| unsafe { &*(b.as_ref() as *const dyn PlatformCapability) })
    }

    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.handlers.read().unwrap().keys().cloned().collect()
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self { Self::new() }
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
pub struct TestPlatformCap {
    pub id: CapabilityId,
    pub min_profile: Profile,
}

impl foundation_wasm::WasmCapability for TestPlatformCap {
    fn name(&self) -> &str { &self.id.0 }

    fn invoke_capability(
        &self,
        request: &CapabilityRequest<Vec<u8>>,
    ) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError> {
        Ok(CapabilityResponse {
            capability: request.capability.clone(),
            action: request.action.clone(),
            payload: request.payload.clone(),
            content_type: request.content_type,
        })
    }
}

impl PlatformCapability for TestPlatformCap {
    fn capability_id(&self) -> &CapabilityId { &self.id }
    fn min_profile(&self) -> Profile { self.min_profile }
}

/// Create a test registry with a camera capability registered.
#[must_use]
pub fn test_registry() -> CapabilityRegistry {
    let reg = CapabilityRegistry::new();
    reg.register(TestPlatformCap {
        id: CapabilityId("camera".into()),
        min_profile: Profile::TrustedRemote,
    });
    reg
}
