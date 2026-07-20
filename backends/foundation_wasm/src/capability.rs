//! Platform-agnostic capability primitives.
//!
//! WHY: Capabilities (camera, clipboard, filesystem, biometrics) need a
//! portable trait + registry that works on wasm32 and native. The platform
//! (`foundation_platform`) wraps this with Tauri-specific security (profile
//! gates, per-route allowlists, stale-page guards). Non-platform hosts
//! (browser, Deno, testbed) get a plain registry with no overhead.
//!
//! WHAT: `WasmCapability` trait, `CapabilityRegistry`, and the generic
//! `CapabilityRequest<T>`/`CapabilityResponse<T>` wire types. The default
//! `T = Vec<u8>` is the on-wire format. Any `T: WirePayload` can round-trip
//! through `into_wire()` / `into_typed()` — higher-level crates enforce
//! `serde::Serialize + Deserialize` or `ToArrow + FromArrow` on `T`.
//!
//! HOW: `no_std` compatible — uses `alloc` types. On native: `Send + Sync`
//! trait bounds + `Mutex`. On wasm32: no thread bounds, single-threaded.

use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_nostd::comp::basic::Mutex;

// ── WirePayload trait ───────────────────────────────────────────────────

/// Content type for capability payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityContentType {
    /// JSON-encoded payload (`application/json`).
    Json,
    /// Arrow columnar payload (`application/vnd.apache.arrow.batch`).
    Arrow,
    /// Raw binary payload (`application/octet-stream`).
    Binary,
}

/// Trait for types that can be serialised onto / deserialised from the wire.
///
/// Higher-level crates provide blanket impls: anything `serde::Serialize +
/// serde::DeserializeOwned` gets `CapabilityContentType::Json`. Anything
/// `ToArrow + FromArrow` gets `CapabilityContentType::Arrow`.
///
/// `Vec<u8>` has a built-in impl (passthrough, `CapabilityContentType::Json`
/// by default — callers should set `content_type` explicitly when building).
pub trait WirePayload: Sized {
    /// Serialise `self` into wire bytes and declare its content type.
    fn into_wire_bytes(self) -> (Vec<u8>, CapabilityContentType);

    /// Deserialise from wire bytes with a known content type.
    fn from_wire_bytes(data: &[u8], content_type: CapabilityContentType)
        -> Result<Self, WireError>;
}

impl WirePayload for Vec<u8> {
    fn into_wire_bytes(self) -> (Vec<u8>, CapabilityContentType) {
        // Default to Json — the caller should set the correct content_type
        // on the request/response struct before serialising if it isn't JSON.
        (self, CapabilityContentType::Json)
    }

    fn from_wire_bytes(
        data: &[u8],
        _content_type: CapabilityContentType,
    ) -> Result<Self, WireError> {
        Ok(data.to_vec())
    }
}

/// Error during wire serialisation / deserialisation.
#[derive(Debug, Clone)]
pub enum WireError {
    /// The payload could not be encoded.
    EncodeFailed(String),
    /// The payload could not be decoded.
    DecodeFailed(String),
    /// Unsupported content type for this codec.
    UnsupportedContentType(CapabilityContentType),
}

// ── Wire types (generic over payload T) ─────────────────────────────────

/// A serialized capability invocation request.
///
/// Default `T = Vec<u8>` is the on-wire form. Any `T: WirePayload` can
/// go onto the wire via `self.into_wire()` (which calls
/// `WirePayload::into_wire_bytes` on the payload and updates `content_type`).
#[derive(Debug, Clone)]
pub struct CapabilityRequest<T = Vec<u8>> {
    /// Matches a registered `WasmCapability::name()`.
    pub capability: String,
    /// The action to perform (e.g. "read", "write", "capture").
    pub action: String,
    /// The typed payload.
    pub payload: T,
    /// Payload encoding.
    pub content_type: CapabilityContentType,
}

/// The result of a capability invocation.
#[derive(Debug, Clone)]
pub struct CapabilityResponse<T = Vec<u8>> {
    /// Echoes the request capability for correlation.
    pub capability: String,
    /// Echoes the request action.
    pub action: String,
    /// The typed result.
    pub payload: T,
    /// Result encoding.
    pub content_type: CapabilityContentType,
}

// ── into_wire / into_typed on requests ─────────────────────────────────

impl<T: WirePayload> CapabilityRequest<T> {
    /// Convert a typed request into the wire form.
    ///
    /// Calls `WirePayload::into_wire_bytes(self.payload)` and sets
    /// `content_type` from the result.
    pub fn into_wire(self) -> CapabilityRequest<Vec<u8>> {
        let (payload, content_type) = self.payload.into_wire_bytes();
        CapabilityRequest {
            capability: self.capability,
            action: self.action,
            payload,
            content_type,
        }
    }

    /// Create a wire request from typed parts. Convenience: equivalent to
    /// `CapabilityRequest { .. }.into_wire()`.
    pub fn wire(
        capability: impl Into<String>,
        action: impl Into<String>,
        payload: T,
    ) -> CapabilityRequest<Vec<u8>> {
        CapabilityRequest {
            capability: capability.into(),
            action: action.into(),
            payload,
            content_type: CapabilityContentType::Json,
        }
        .into_wire()
    }
}

impl CapabilityRequest<Vec<u8>> {
    /// Convert a wire request back into a typed request.
    ///
    /// Calls `WirePayload::from_wire_bytes`.
    pub fn into_typed<T: WirePayload>(self) -> Result<CapabilityRequest<T>, WireError> {
        let payload = T::from_wire_bytes(&self.payload, self.content_type)?;
        Ok(CapabilityRequest {
            capability: self.capability,
            action: self.action,
            payload,
            content_type: self.content_type,
        })
    }
}

// ── into_wire / into_typed on responses ────────────────────────────────

impl<T: WirePayload> CapabilityResponse<T> {
    /// Convert a typed response into the wire form.
    pub fn into_wire(self) -> CapabilityResponse<Vec<u8>> {
        let (payload, content_type) = self.payload.into_wire_bytes();
        CapabilityResponse {
            capability: self.capability,
            action: self.action,
            payload,
            content_type,
        }
    }

    /// Create a wire response from typed parts.
    pub fn wire(
        capability: impl Into<String>,
        action: impl Into<String>,
        payload: T,
    ) -> CapabilityResponse<Vec<u8>> {
        CapabilityResponse {
            capability: capability.into(),
            action: action.into(),
            payload,
            content_type: CapabilityContentType::Json,
        }
        .into_wire()
    }
}

impl CapabilityResponse<Vec<u8>> {
    /// Convert a wire response back into a typed response.
    pub fn into_typed<T: WirePayload>(self) -> Result<CapabilityResponse<T>, WireError> {
        let payload = T::from_wire_bytes(&self.payload, self.content_type)?;
        Ok(CapabilityResponse {
            capability: self.capability,
            action: self.action,
            payload,
            content_type: self.content_type,
        })
    }
}

// ── Errors ──────────────────────────────────────────────────────────────

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

impl From<WireError> for CapabilityError {
    fn from(e: WireError) -> Self {
        match e {
            WireError::DecodeFailed(msg) => CapabilityError::InvalidPayload(msg),
            WireError::EncodeFailed(msg) => CapabilityError::ExecutionFailed(msg),
            WireError::UnsupportedContentType(ct) => {
                CapabilityError::InvalidPayload(format!("unsupported content type: {ct:?}"))
            }
        }
    }
}

// ── Capability trait ────────────────────────────────────────────────────

/// A portable capability that can be invoked from WASM or native code.
///
/// Works with the wire format (`CapabilityRequest<Vec<u8>>`). Higher-level
/// crates can implement typed wrappers that convert via `WirePayload`.
///
/// On native targets: `Send + Sync`. On wasm32: no bounds (single-threaded).
#[cfg(target_family = "wasm")]
pub trait WasmCapability {
    /// Unique capability name. Used as the lookup key.
    fn name(&self) -> &str;

    /// Invoke the capability with a wire request.
    fn invoke_capability(
        &self,
        request: &CapabilityRequest<Vec<u8>>,
    ) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError>;
}

#[cfg(not(target_family = "wasm"))]
pub trait WasmCapability: Send + Sync {
    /// Unique capability name. Used as the lookup key.
    fn name(&self) -> &str;

    /// Invoke the capability with a wire request.
    fn invoke_capability(
        &self,
        request: &CapabilityRequest<Vec<u8>>,
    ) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError>;
}

// ── Registry ────────────────────────────────────────────────────────────

/// Internal storage: `dyn WasmCapability` behind a Box.
///
/// On native: `Send + Sync`. On wasm32: plain `'static`.
#[cfg(target_family = "wasm")]
type DynCapability = Box<dyn WasmCapability + 'static>;

#[cfg(not(target_family = "wasm"))]
type DynCapability = Box<dyn WasmCapability + Send + Sync + 'static>;

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
    pub fn register(&self, capability: impl WasmCapability + 'static) -> Option<DynCapability> {
        let mut map = self
            .inner
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        map.insert(String::from(capability.name()), Box::new(capability))
    }

    #[cfg(target_family = "wasm")]
    pub fn register(&mut self, capability: impl WasmCapability + 'static) -> Option<DynCapability> {
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
        #[cfg(not(target_family = "wasm"))]
        {
            let guard = self
                .inner
                .lock()
                .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
            guard
                .get(name)
                .map(|b| unsafe { &*core::ptr::from_ref::<dyn WasmCapability>(b.as_ref()) })
        }
        #[cfg(target_family = "wasm")]
        {
            self.inner
                .get(name)
                .map(|b| b.as_ref() as &dyn WasmCapability)
        }
    }

    /// Invoke a capability by name with a wire request.
    ///
    /// Returns `CapabilityError::UnknownCapability` if not registered.
    ///
    /// # Panics
    ///
    /// On native: panics if the mutex is poisoned.
    pub fn invoke(
        &self,
        request: &CapabilityRequest<Vec<u8>>,
    ) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError> {
        match self.get(&request.capability) {
            Some(cap) => cap.invoke_capability(request),
            None => Err(CapabilityError::UnknownCapability(
                request.capability.clone(),
            )),
        }
    }

    /// Invoke a capability by name with a typed request.
    ///
    /// Converts through `into_wire()` → invoke → `into_typed()`.
    pub fn invoke_typed<T: WirePayload>(
        &self,
        request: CapabilityRequest<T>,
    ) -> Result<CapabilityResponse<T>, CapabilityError> {
        let wire_req = request.into_wire();
        let wire_resp = self.invoke(&wire_req)?;
        Ok(wire_resp.into_typed()?)
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
