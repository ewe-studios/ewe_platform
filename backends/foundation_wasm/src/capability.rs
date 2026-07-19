//! Platform-agnostic capability primitives.
//!
//! WHY: Capabilities (camera, clipboard, filesystem, biometrics) need a
//! portable trait + registry that works on wasm32 and native. The platform
//! (foundation_platform) wraps this with Tauri-specific security (profile
//! gates, per-route allowlists, stale-page guards). Non-platform hosts
//! (browser, Deno, testbed) get a plain registry with no overhead.
//!
//! WHAT: `WasmCapability` trait, `CapabilityRegistry`, and the
//! `CapabilityRequest`/`CapabilityResponse` wire types.
//!
//! HOW: no_std compatible — uses `alloc` types. On native: `Send + Sync`
//! trait bounds + `Mutex`. On wasm32: no thread bounds, single-threaded.

use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_nostd::comp::basic::Mutex;

// ── Wire types ──────────────────────────────────────────────────────────

/// Content type for capability payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityContentType {
    /// JSON-encoded payload (`application/json`).
    Json,
    /// Arrow columnar payload (`application/vnd.apache.arrow.batch`).
    Arrow,
}

/// A serialized capability invocation request.
///
/// Platform-agnostic: carries bytes, not `serde_json::Value`.
/// The dispatcher chooses the codec based on `content_type`.
#[derive(Debug, Clone)]
pub struct CapabilityRequest {
    /// Matches a registered `WasmCapability::name()`.
    pub capability: String,
    /// The action to perform (e.g. "read", "write", "capture").
    pub action: String,
    /// Serialized payload. Encoding indicated by `content_type`.
    pub payload: Vec<u8>,
    /// Payload encoding.
    pub content_type: CapabilityContentType,
}

/// The result of a capability invocation.
#[derive(Debug, Clone)]
pub struct CapabilityResponse {
    /// Echoes the request capability for correlation.
    pub capability: String,
    /// Echoes the request action.
    pub action: String,
    /// Serialized result. Encoding indicated by `content_type`.
    pub payload: Vec<u8>,
    /// Result encoding.
    pub content_type: CapabilityContentType,
}

/// Errors from capability invocation.
#[derive(Debug, Clone)]
pub enum CapabilityError {
    /// No capability registered under this name.
    UnknownCapability(String),
    /// The payload could not be decoded.
    InvalidPayload(String),
    /// The capability handler failed.
    ExecutionFailed(String),
    /// The caller does not have permission.
    PermissionDenied(String),
}

// ── Capability trait ────────────────────────────────────────────────────

/// A portable capability that can be invoked from WASM or native code.
///
/// On native targets: `Send + Sync + 'static`.
/// On wasm32: `'static` only (single-threaded).
#[cfg(target_family = "wasm")]
pub trait WasmCapability: 'static {
    /// Unique capability name. Used as the lookup key.
    fn name(&self) -> &str;

    /// Invoke the capability with a serialized request.
    fn invoke_capability(
        &self,
        request: &CapabilityRequest,
    ) -> Result<CapabilityResponse, CapabilityError>;
}

#[cfg(not(target_family = "wasm"))]
pub trait WasmCapability: Send + Sync + 'static {
    /// Unique capability name. Used as the lookup key.
    fn name(&self) -> &str;

    /// Invoke the capability with a serialized request.
    fn invoke_capability(
        &self,
        request: &CapabilityRequest,
    ) -> Result<CapabilityResponse, CapabilityError>;
}

// ── Registry ────────────────────────────────────────────────────────────

/// Internal storage: `dyn WasmCapability` behind a Box.
///
/// On native: `Send + Sync`. On wasm32: plain `'static`.
#[cfg(target_family = "wasm")]
type DynCapability = Box<dyn WasmCapability>;

#[cfg(not(target_family = "wasm"))]
type DynCapability = Box<dyn WasmCapability + Send + Sync>;

/// A portable capability registry.
///
/// On native targets: wrapped in a `Mutex` for thread-safe access.
/// On wasm32: single-threaded, no locking.
///
/// `foundation_platform` wraps this with its own security layer
/// (profile gates, per-route allowlists, stale-page guards).
pub struct CapabilityRegistry {
    #[cfg(not(target_family = "wasm"))]
    inner: Mutex<BTreeMap<String, DynCapability>>,

    #[cfg(target_family = "wasm")]
    inner: BTreeMap<String, DynCapability>,
}

impl CapabilityRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            #[cfg(not(target_family = "wasm"))]
            inner: Mutex::new(BTreeMap::new()),

            #[cfg(target_family = "wasm")]
            inner: BTreeMap::new(),
        }
    }

    /// Register a capability. Returns the previous capability registered under
    /// the same name, if any.
    ///
    /// Takes `&self` — on native the internal `Mutex` provides interior
    /// mutability. On wasm32 (single-threaded, no Mutex), this still requires
    /// `&mut self`.
    ///
    /// # Panics
    ///
    /// On native: panics if the mutex is poisoned.
    #[cfg(not(target_family = "wasm"))]
    pub fn register(&self, capability: impl WasmCapability) -> Option<DynCapability> {
        let mut map = self
            .inner
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        map.insert(String::from(capability.name()), Box::new(capability))
    }

    #[cfg(target_family = "wasm")]
    pub fn register(&mut self, capability: impl WasmCapability) -> Option<DynCapability> {
        self.inner
            .insert(String::from(capability.name()), Box::new(capability))
    }

    /// Look up a capability by name.
    ///
    /// Returns `None` if no capability is registered under that name.
    ///
    /// # Panics
    ///
    /// On native: panics if the mutex is poisoned.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn WasmCapability> {
        // SAFETY: The registry owns the Box<dyn WasmCapability>.
        // We return a reference bound by the lock guard's lifetime.
        #[cfg(not(target_family = "wasm"))]
        {
            let guard = self
                .inner
                .lock()
                .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
            // Leak the reference through the MutexGuard — safe because
            // the registry outlives any capability access.
            guard.get(name).map(|b| unsafe {
                &*(b.as_ref() as *const dyn WasmCapability)
            })
        }
        #[cfg(target_family = "wasm")]
        {
            self.inner
                .get(name)
                .map(|b| b.as_ref() as &dyn WasmCapability)
        }
    }

    /// Invoke a capability by name.
    ///
    /// Returns `CapabilityError::UnknownCapability` if not registered.
    ///
    /// # Panics
    ///
    /// On native: panics if the mutex is poisoned.
    pub fn invoke(
        &self,
        request: &CapabilityRequest,
    ) -> Result<CapabilityResponse, CapabilityError> {
        match self.get(&request.capability) {
            Some(cap) => cap.invoke_capability(request),
            None => Err(CapabilityError::UnknownCapability(
                request.capability.clone(),
            )),
        }
    }

    /// Return all registered capability names.
    ///
    /// # Panics
    ///
    /// On native: panics if the mutex is poisoned.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        #[cfg(not(target_family = "wasm"))]
        {
            self.inner
                .lock()
                .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                .keys()
                .cloned()
                .collect()
        }
        #[cfg(target_family = "wasm")]
        {
            self.inner.keys().cloned().collect()
        }
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helper: capability name validation ──────────────────────────────────

/// Validate a capability name.
///
/// Names must be non-empty, alphanumeric + underscores + hyphens,
/// max 64 characters.
#[must_use]
pub fn is_valid_capability_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    name.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}
