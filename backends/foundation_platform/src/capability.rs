//! Platform IPC security layer (F41 — was F05/F23 capability registry).
//!
//! Wraps the unified `foundation_wasm::ipc::Ipc` trait with platform-specific
//! security checks. The old `PlatformCapability` trait extended `WasmCapability`;
//! now it extends `Ipc<Vec<u8>, Vec<u8>>` directly and adds the 5-layer defense.
//!
//! 5-layer defense: profile gate → registration → per-route allowlist →
//! OS permission → stale-page guard.

use std::collections::HashMap;
use std::sync::RwLock;

use foundation_ui_traits::{CapabilityId, PageIdentity, Profile, RouteDecision};
use foundation_wasm::ipc::{Ipc, IpcContentType, IpcError, IpcKind, IpcRequest, IpcResponse};

use crate::profiles::{Access, ProfileGate, Service};
use crate::session::PlatformSession;

// ── PlatformIpc trait (was PlatformCapability) ──────────────────────────

/// A platform-native IPC handler. Extends `foundation_wasm::ipc::Ipc`.
///
/// Replaces the old `PlatformCapability` (F23) which extended `WasmCapability`.
/// Now extends `Ipc<Vec<u8>, Vec<u8>>` directly. The 5-layer defense is
/// applied by `PlatformIpcRegistry::invoke()`.
///
/// Handlers that need OS-level access (camera, biometrics) additionally
/// implement `AndroidIpc` or `IosIpc` (see `handle.rs`).
pub trait PlatformIpc: Ipc<Vec<u8>, Vec<u8>> {
    /// The platform capability ID (e.g. "camera", "biometric_auth").
    fn capability_id(&self) -> &CapabilityId;

    /// Minimum WebView profile required to invoke this handler.
    fn min_profile(&self) -> Profile;

    /// Invoke with session context (F43: callback-based).
    fn invoke_with_session(
        &self,
        _session: &PlatformSession,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let _ = _session;
        Ipc::invoke(self, request)
    }

    /// Callback variant — default routes through the sync invoke.
    fn invoke_with_session_cb(
        &self,
        session: &PlatformSession,
        request: &IpcRequest<Vec<u8>>,
        callback: Box<dyn FnOnce(Result<IpcResponse<Vec<u8>>, IpcError>) + Send>,
    ) {
        let result = self.invoke_with_session(session, request);
        callback(result);
    }
}

// ── PlatformIpcRegistry (was CapabilityRegistry) ────────────────────────

/// Registry of platform IPC handlers with the full 5-layer defense chain.
///
/// Each registered handler implements both `Ipc<Vec<u8>, Vec<u8>>` (wire format)
/// and `PlatformIpc` (security metadata). Replaces the old `CapabilityRegistry`.
pub struct PlatformIpcRegistry {
    handlers: RwLock<HashMap<String, Box<dyn PlatformIpc>>>,
}

impl PlatformIpcRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a platform IPC handler.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn register(&self, cap: impl PlatformIpc + 'static) {
        let name = cap.capability_id().0.clone();
        self.handlers.write().unwrap().insert(name, Box::new(cap));
    }

    /// Invoke through the 5-layer defense chain. Callback fires with result.
    pub fn invoke<F>(
        &self,
        session: &PlatformSession,
        request: &IpcRequest<Vec<u8>>,
        page_identity: &PageIdentity,
        current_route: Option<&RouteDecision>,
        callback: F,
    ) -> Result<(), IpcError>
    where
        F: FnOnce(Result<IpcResponse<Vec<u8>>, IpcError>) + Send + 'static,
    {
        // Layer 1: Stale-page guard
        if !session.is_active_page(page_identity) {
            return Err(IpcError::PermissionDenied(
                "stale page — request from navigated-away page".into(),
            ));
        }

        let guard = self.handlers.read().unwrap();

        let handler = guard
            .get(&request.ipc)
            .ok_or_else(|| IpcError::UnknownIpc(request.ipc.clone()))?;

        let profile = current_route
            .map(|r| r.profile)
            .unwrap_or(Profile::UntrustedRemote);
        let gate = ProfileGate::new(profile);
        if let Err(e) = gate.check(Service::NativeApi, Access::Execute) {
            return Err(IpcError::PermissionDenied(e.to_string()));
        }
        if !profile_satisfies(profile, handler.min_profile()) {
            return Err(IpcError::PermissionDenied(format!(
                "profile {profile:?} too low for '{}' (requires {:?})",
                handler.capability_id().0,
                handler.min_profile()
            )));
        }

        if let Some(route) = current_route {
            if !route.capabilities.is_empty()
                && !route
                    .capabilities
                    .iter()
                    .any(|c| c == handler.capability_id())
            {
                return Err(IpcError::PermissionDenied(format!(
                    "'{}' not allowed on this route",
                    handler.capability_id().0
                )));
            }
        }

        handler.invoke_with_session_cb(session, request, Box::new(callback));
        Ok(())
    }

    /// Invoke through the security layer with a JSON payload. Blocks on
    /// a channel internally (sync shim over the callback-based invoke).
    pub fn invoke_json(
        &self,
        session: &PlatformSession,
        ipc_name: &str,
        action: &str,
        payload: &serde_json::Value,
        page_identity: &PageIdentity,
        current_route: Option<&RouteDecision>,
    ) -> Result<serde_json::Value, IpcError> {
        let payload_bytes =
            serde_json::to_vec(payload).map_err(|e| IpcError::InvalidPayload(e.to_string()))?;

        let request = IpcRequest {
            ipc: ipc_name.to_string(),
            action: action.to_string(),
            payload: payload_bytes,
            content_type: IpcContentType::Json,
            target: None,
        };

        let (tx, rx) = std::sync::mpsc::channel();
        self.invoke(session, &request, page_identity, current_route, move |r| {
            let _ = tx.send(r);
        })?;
        let response = rx
            .recv()
            .map_err(|_| IpcError::ExecutionFailed("invoke_json callback dropped".into()))??;
        serde_json::from_slice(&response.payload)
            .map_err(|e| IpcError::InvalidPayload(e.to_string()))
    }

    /// Look up a handler by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn PlatformIpc> {
        let guard = self.handlers.read().unwrap();
        guard
            .get(name)
            .map(|b| unsafe { &*(b.as_ref() as *const dyn PlatformIpc) })
    }

    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.handlers.read().unwrap().keys().cloned().collect()
    }
}

impl Default for PlatformIpcRegistry {
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
        return actual == required || (actual == Profile::Devtools && cfg!(debug_assertions));
    }
    rank(actual) >= rank(required)
}

// ── Test helpers ───────────────────────────────────────────────────────

/// Test IPC handler for integration tests.
pub struct TestPlatformIpc {
    pub id: CapabilityId,
    pub min_profile: Profile,
}

impl Ipc<Vec<u8>, Vec<u8>> for TestPlatformIpc {
    fn name(&self) -> &str {
        &self.id.0
    }
    fn kind(&self) -> IpcKind {
        IpcKind::Capability
    }

    fn invoke(&self, request: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Ok(IpcResponse {
            payload: request.payload.clone(),
            content_type: request.content_type,
        })
    }
}

impl PlatformIpc for TestPlatformIpc {
    fn capability_id(&self) -> &CapabilityId {
        &self.id
    }
    fn min_profile(&self) -> Profile {
        self.min_profile
    }
}

/// Create a test registry with a camera handler registered.
#[must_use]
pub fn test_registry() -> PlatformIpcRegistry {
    let reg = PlatformIpcRegistry::new();
    reg.register(TestPlatformIpc {
        id: CapabilityId("camera".into()),
        min_profile: Profile::TrustedRemote,
    });
    reg
}
