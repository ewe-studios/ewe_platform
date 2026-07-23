//! Unified IPC primitives — merge of F23 (capabilities) + F25 (IPC) + F41 (FFI).
//!
//! WHY: IPC and capabilities were the same shape (name(), invoke(Request)→Response,
//! Box<dyn> registry). Two traits, two registries, two Tauri commands, zero FFI.
//! Everything folds into one generic `Ipc<Input, Output>` trait with a single
//! registry, a single FFI surface, and a typed dispatch helper.
//!
//! WHAT: `WirePayload` trait for typed⇄wire conversion. `IpcContentType` enum.
//! Generic `IpcRequest<T>` / `IpcResponse<T>` with `Vec<u8>` defaults.
//! `Ipc<Input, Output>` trait. `IpcRegistry` (Mutex-guarded on native).
//! `IpcKind::Capability { min_profile }` for security-gated handlers.
//! `IpcError` with `PermissionDenied`. `host_ipc_dispatch()` typed FFI helper.
//!
//! HOW: `no_std` compatible — uses `alloc` types. On native: `Send + Sync` bounds
//! + `Mutex`. On wasm32: single-threaded, no bounds.

use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_nostd::comp::basic::Mutex;

// ── Content type ────────────────────────────────────────────────────────

/// Content type for IPC payloads (was `CapabilityContentType` in F23).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcContentType {
    /// JSON-encoded payload (`application/json`).
    Json,
    /// Arrow columnar payload (`application/vnd.apache.arrow.batch`).
    Arrow,
    /// Raw binary payload (`application/octet-stream`).
    Binary,
}

// ── WirePayload trait ───────────────────────────────────────────────────

/// Trait for types that can be serialised onto / deserialised from the wire.
///
/// Higher-level crates provide blanket impls: anything `serde::Serialize +
/// serde::DeserializeOwned` gets `IpcContentType::Json`. Anything
/// `ToArrow + FromArrow` gets `IpcContentType::Arrow`.
///
/// `Vec<u8>` has a built-in impl (passthrough, `IpcContentType::Json`
/// by default — callers should set `content_type` explicitly when building).
pub trait WirePayload: Sized {
    /// Serialise `self` into wire bytes and declare its content type.
    fn into_wire_bytes(self) -> (Vec<u8>, IpcContentType);

    /// Deserialise from wire bytes with a known content type.
    /// # Errors
    ///
    /// Returns [`WireError`] if deserialisation fails.
    fn from_wire_bytes(data: &[u8], content_type: IpcContentType)
        -> Result<Self, WireError>;
}

impl WirePayload for Vec<u8> {
    fn into_wire_bytes(self) -> (Vec<u8>, IpcContentType) {
        (self, IpcContentType::Json)
    }

    fn from_wire_bytes(
        data: &[u8],
        _content_type: IpcContentType,
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
    UnsupportedContentType(IpcContentType),
}

// ── Wire types (generic over payload T) ─────────────────────────────────

/// A request from the frontend to an IPC handler.
///
/// Default `T = Vec<u8>` is the on-wire form. Any `T: WirePayload` can
/// go onto the wire via `self.into_wire()`.
#[derive(Debug, Clone)]
pub struct IpcRequest<T = Vec<u8>> {
    /// The IPC name (matches a registered `Ipc::name()`).
    pub ipc: String,
    /// The action or sub-command within this IPC.
    pub action: String,
    /// The typed payload.
    pub payload: T,
    /// Payload encoding.
    pub content_type: IpcContentType,
    /// For Emit IPCs: the target webview label, or None for broadcast.
    pub target: Option<String>,
}

/// The response from an IPC handler.
#[derive(Debug, Clone)]
pub struct IpcResponse<T = Vec<u8>> {
    /// The typed result.
    pub payload: T,
    /// Result encoding.
    pub content_type: IpcContentType,
}

// ── into_wire / into_typed on requests ─────────────────────────────────

impl<T: WirePayload> IpcRequest<T> {
    /// Convert a typed request into the wire form.
    pub fn into_wire(self) -> IpcRequest<Vec<u8>> {
        let (payload, content_type) = self.payload.into_wire_bytes();
        IpcRequest {
            ipc: self.ipc,
            action: self.action,
            payload,
            content_type,
            target: self.target,
        }
    }

    /// Create a wire request from typed parts.
    pub fn wire(ipc: impl Into<String>, action: impl Into<String>, payload: T) -> IpcRequest<Vec<u8>> {
        IpcRequest {
            ipc: ipc.into(),
            action: action.into(),
            payload,
            content_type: IpcContentType::Json,
            target: None,
        }.into_wire()
    }
}

impl IpcRequest<Vec<u8>> {
    /// Convert a wire request back into a typed request.
    /// # Errors
    ///
    /// Returns [`WireError`] if deserialisation fails.
    pub fn into_typed<T: WirePayload>(self) -> Result<IpcRequest<T>, WireError> {
        let payload = T::from_wire_bytes(&self.payload, self.content_type)?;
        Ok(IpcRequest {
            ipc: self.ipc,
            action: self.action,
            payload,
            content_type: self.content_type,
            target: self.target,
        })
    }
}

// ── into_wire / into_typed on responses ────────────────────────────────

impl<T: WirePayload> IpcResponse<T> {
    /// Convert a typed response into the wire form.
    pub fn into_wire(self) -> IpcResponse<Vec<u8>> {
        let (payload, content_type) = self.payload.into_wire_bytes();
        IpcResponse { payload, content_type }
    }

    /// Create a wire response from typed parts.
    pub fn wire(payload: T) -> IpcResponse<Vec<u8>> {
        IpcResponse {
            payload,
            content_type: IpcContentType::Json,
        }.into_wire()
    }
}

impl IpcResponse<Vec<u8>> {
    /// Convert a wire response back into a typed response.
    /// # Errors
    ///
    /// Returns [`WireError`] if deserialisation fails.
    pub fn into_typed<T: WirePayload>(self) -> Result<IpcResponse<T>, WireError> {
        let payload = T::from_wire_bytes(&self.payload, self.content_type)?;
        Ok(IpcResponse { payload, content_type: self.content_type })
    }
}

// ── Errors ─────────────────────────────────────────────────────────────

/// Errors from IPC invocation. Absorbed `CapabilityError` (F23) variants.
#[derive(Debug, Clone)]
pub enum IpcError {
    /// No IPC registered under this name.
    UnknownIpc(String),
    /// The payload could not be decoded.
    InvalidPayload(String),
    /// The IPC handler failed.
    ExecutionFailed(String),
    /// The caller does not have permission (was: `CapabilityError::PermissionDenied`).
    PermissionDenied(String),
    /// Domain-level error with a machine-readable code.
    DomainError { code: String, message: String },
}

impl From<WireError> for IpcError {
    fn from(e: WireError) -> Self {
        match e {
            WireError::DecodeFailed(msg) => IpcError::InvalidPayload(msg),
            WireError::EncodeFailed(msg) => IpcError::ExecutionFailed(msg),
            WireError::UnsupportedContentType(ct) => {
                IpcError::InvalidPayload(format!("unsupported content type: {ct:?}"))
            }
        }
    }
}

// ── IpcKind ─────────────────────────────────────────────────────────────

/// The transport kind for an IPC handler.
///
/// Streaming is NOT a variant — streaming needs a different contract
/// (bidirectional chunks, backpressure). See `StreamingIpc` (F26).
///
/// The transport kind for an IPC handler.
///
/// `Capability` tags the handler as security-gated. The platform's security
/// layer (foundation_platform) reads this tag and applies the 5-layer defense
/// (profile gate → registration → per-route allowlist → OS permission →
/// stale-page guard). The min_profile is stored on the platform-side handler,
/// not here — this crate is dependency-free and has no Profile type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcKind {
    /// Request → Response. Single reply (like Tauri invoke).
    Query,
    /// Fire-and-forget. No response expected (like Tauri events).
    Emit,
    /// Request → Response, rendered as a page. Can also serve as an
    /// HTTP route handler when registered with a platform that supports it.
    Page,
    /// A security-gated native capability. Tag only — the platform
    /// security layer enforces the actual profile gate.
    /// Replaces the old `WasmCapability` + `PlatformCapability` split (F23).
    Capability,
}

// ── Ipc trait ───────────────────────────────────────────────────────────

/// A unified IPC handler. Works on wasm32 and native. All capability
/// invocations are IPC invocations.
///
/// Generic over `Input`/`Output` — defaults to `Vec<u8>` for the wire format
/// stored in registries. Concrete handlers use domain types to express their
/// delivery model:
///
/// | Output type | Delivery model |
/// |---|---|
/// | `Vec<u8>` | Wire bytes, sync |
/// | `SomeStruct` | Typed data, sync |
/// | `u64` (stream ID) | Streaming — handler creates a HostStreamQueue |
/// | `()` | Fire-and-forget, no response expected |
///
/// On native targets: `Send + Sync`. On wasm32: no bounds (single-threaded).
#[cfg(target_family = "wasm")]
pub trait Ipc<Input = Vec<u8>, Output = Vec<u8>> {
    /// Unique handler name. Used as the lookup key in registries.
    fn name(&self) -> &str;

    /// The transport kind (Query, Emit, Page, Capability).
    fn kind(&self) -> IpcKind;

    /// Invoke the handler with a request.
    /// # Errors
    ///
    /// Returns [`IpcError`] if invocation fails.
    fn invoke(
        &self,
        request: &IpcRequest<Input>,
    ) -> Result<IpcResponse<Output>, IpcError>;
}

#[cfg(not(target_family = "wasm"))]
pub trait Ipc<Input = Vec<u8>, Output = Vec<u8>>: Send + Sync {
    /// Unique handler name. Used as the lookup key in registries.
    fn name(&self) -> &str;

    /// The transport kind (Query, Emit, Page, Capability).
    fn kind(&self) -> IpcKind;

    /// Invoke the handler with a request.
    /// # Errors
    ///
    /// Returns [`IpcError`] if invocation fails.
    fn invoke(
        &self,
        request: &IpcRequest<Input>,
    ) -> Result<IpcResponse<Output>, IpcError>;
}

// ── Registry ────────────────────────────────────────────────────────────

/// Internal storage: `dyn Ipc` behind a Box.
///
/// On native: `Send + Sync`. On wasm32: plain `'static`.
#[cfg(target_family = "wasm")]
type DynIpc = Box<dyn Ipc<Vec<u8>, Vec<u8>> + 'static>;

#[cfg(not(target_family = "wasm"))]
type DynIpc = Box<dyn Ipc<Vec<u8>, Vec<u8>> + Send + Sync + 'static>;

/// A portable IPC registry. Unified — replaces both the old `IpcRegistry`
/// (F25) and `CapabilityRegistry` (F23).
///
/// On native targets: wrapped in a `Mutex` for thread-safe access.
/// On wasm32: single-threaded, no locking.
///
/// `foundation_platform` wraps this with its own security layer
/// (profile gates, per-route allowlists, stale-page guards) and the
/// two-tier native/pure dispatch.
pub struct IpcRegistry {
    #[cfg(not(target_family = "wasm"))]
    inner: Mutex<BTreeMap<String, DynIpc>>,

    #[cfg(target_family = "wasm")]
    inner: BTreeMap<String, DynIpc>,
}

impl IpcRegistry {
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

    /// Register an IPC handler. Returns the previous handler registered under
    /// the same name, if any.
    ///
    /// Takes `&self` — on native the internal `Mutex` provides interior
    /// mutability. On wasm32 (single-threaded, no Mutex), this requires
    /// `&mut self`.
    ///
    /// # Panics
    ///
    /// On native: panics if the mutex is poisoned.
    #[cfg(not(target_family = "wasm"))]
    pub fn register(&self, ipc: impl Ipc<Vec<u8>, Vec<u8>> + 'static) -> Option<DynIpc> {
        let mut map = self
            .inner
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        map.insert(String::from(ipc.name()), Box::new(ipc))
    }

    #[cfg(target_family = "wasm")]
    pub fn register(&mut self, ipc: impl Ipc<Vec<u8>, Vec<u8>> + 'static) -> Option<DynIpc> {
        self.inner
            .insert(String::from(ipc.name()), Box::new(ipc))
    }

    /// Look up an IPC handler by name.
    ///
    /// Returns `None` if no handler is registered under that name.
    ///
    /// # Panics
    ///
    /// On native: panics if the mutex is poisoned.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn Ipc<Vec<u8>, Vec<u8>>> {
        #[cfg(not(target_family = "wasm"))]
        {
            let guard = self
                .inner
                .lock()
                .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
            guard
                .get(name)
                .map(|b| unsafe { &*core::ptr::from_ref::<dyn Ipc<Vec<u8>, Vec<u8>>>(b.as_ref()) })
        }
        #[cfg(target_family = "wasm")]
        {
            self.inner
                .get(name)
                .map(|b| b.as_ref() as &dyn Ipc<Vec<u8>, Vec<u8>>)
        }
    }

    /// Invoke an IPC handler by name with a wire request.
    ///
    /// Returns `IpcError::UnknownIpc` if not registered.
    ///
    /// # Panics
    ///
    /// On native: panics if the mutex is poisoned.
    /// # Errors
    ///
    /// Returns [`IpcError`] if the handler is not registered or
    /// invocation fails.
    pub fn invoke(
        &self,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        match self.get(&request.ipc) {
            Some(ipc) => ipc.invoke(request),
            None => Err(IpcError::UnknownIpc(
                request.ipc.clone(),
            )),
        }
    }

    /// Invoke an IPC handler by name with a typed request.
    ///
    /// Converts through `into_wire()` → invoke → `into_typed()`.
    ///
    /// # Errors
    ///
    /// Returns [`IpcError`] if the handler is not found or
    /// invocation (including wire conversion) fails.
    pub fn invoke_typed<T: WirePayload, U: WirePayload>(
        &self,
        request: IpcRequest<T>,
    ) -> Result<IpcResponse<U>, IpcError> {
        let wire_req = request.into_wire();
        let wire_resp = self.invoke(&wire_req)?;
        Ok(wire_resp.into_typed()?)
    }

    /// Return all registered IPC names.
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

impl Default for IpcRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helper: IPC name validation ──────────────────────────────────────────

/// Validate an IPC / capability name.
///
/// Names must be non-empty, alphanumeric + underscores + hyphens,
/// max 64 characters.
#[must_use]
pub fn is_valid_ipc_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    name.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

// ── FFI: WASM↔Host bindings (F41 Part B) ────────────────────────────────

/// Typed dispatch helper — serialize → host_ipc_invoke → deserialize.
/// Hides the FFI plumbing behind `WirePayload`. Same function for ALL IPCs.
///
/// On wasm32: calls `host_ipc_invoke` via the "platform" import module.
/// On native: panics — native code uses the registry directly.
#[cfg(target_family = "wasm")]
pub fn host_ipc_dispatch<T: WirePayload, U: WirePayload>(
    _name: &str,
    _request: &IpcRequest<T>,
) -> Result<IpcResponse<U>, IpcError> {
    // Stub — the real implementation serializes to bytes, calls
    // host_ipc_invoke(ptr, len), reads the allocation, deserializes.
    // Implemented when the JS host bridge is in place (F41 Part B impl).
    Err(IpcError::ExecutionFailed(
        "host_ipc_dispatch: FFI bridge not yet implemented".into(),
    ))
}

/// WASM imports — the WASM side calls these, every host implements them.
#[cfg(target_family = "wasm")]
mod ffi_imports {
    #[link(wasm_import_module = "platform")]
    extern "C" {
        /// Invoke a named IPC handler on the host. Returns an allocation ID.
        /// 0 = error. Non-zero = allocation containing the serialized
        /// IpcResponse bytes.
        pub fn host_ipc_invoke(
            request_ptr: *const u8,
            request_len: u32,
        ) -> u64;

        /// Open a host→WASM stream for this IPC. Returns a stream ID.
        /// 0 = error. WASM polls chunks via host_ipc_stream_read(id).
        pub fn host_ipc_stream_open(
            request_ptr: *const u8,
            request_len: u32,
        ) -> u64;

        /// Read the next chunk from a host-created stream.
        /// Returns allocation_id (0 = stream closed or error).
        pub fn host_ipc_stream_read(stream_id: u64) -> u64;

        /// Close a host→WASM stream.
        pub fn host_ipc_stream_close(stream_id: u64);
    }
}

#[cfg(target_family = "wasm")]
pub use ffi_imports::*;

/// WASM export — the host calls this to push events to the WASM app.
/// Used for: native chrome events (toolbar button tap), push notifications,
/// deep links, capability reverse events.
///
/// Returns 0 on failure, non-zero on success.
#[cfg(target_family = "wasm")]
#[no_mangle]
pub extern "C" fn ipc_handle_event(
    _event_ptr: *const u8,
    _event_len: u32,
) -> u64 {
    // Stub — dispatched by the host runtime when the JS bridge calls
    // this export. The runtime looks up the event in its internal
    // registry and delivers it to the registered handler.
    0
}

// ── Backward-compatible type aliases ────────────────────────────────────

/// Deprecated alias for `IpcContentType`. Use `IpcContentType` directly.
#[deprecated(note = "use IpcContentType")]
pub type CapabilityContentType = IpcContentType;

/// Deprecated alias for `IpcRequest`. Use `IpcRequest` directly.
#[deprecated(note = "use IpcRequest")]
pub type CapabilityRequest<T = Vec<u8>> = IpcRequest<T>;

/// Deprecated alias for `IpcResponse`. Use `IpcResponse` directly.
#[deprecated(note = "use IpcResponse")]
pub type CapabilityResponse<T = Vec<u8>> = IpcResponse<T>;

/// Deprecated alias for `IpcError`. Use `IpcError` directly.
#[deprecated(note = "use IpcError")]
pub type CapabilityError = IpcError;

/// Deprecated alias for `is_valid_ipc_name`. Use `is_valid_ipc_name` directly.
#[deprecated(note = "use is_valid_ipc_name")]
pub fn is_valid_capability_name(name: &str) -> bool {
    is_valid_ipc_name(name)
}

/// Deprecated re-export. `WasmCapability` is merged into `Ipc`.
/// Existing code: replace `impl WasmCapability for Foo` with `impl Ipc for Foo`.
#[deprecated(note = "use Ipc<Vec<u8>, Vec<u8>>")]
pub trait WasmCapability {
    fn name(&self) -> &str;
    fn invoke_capability(
        &self,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError>;
}

// Blanket impl: any Ipc is a WasmCapability (for backward compat).
#[allow(deprecated)]
impl<T: Ipc<Vec<u8>, Vec<u8>>> WasmCapability for T {
    fn name(&self) -> &str { Ipc::name(self) }
    fn invoke_capability(
        &self,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Ipc::invoke(self, request)
    }
}

/// Deprecated alias for `IpcRegistry`. Use `IpcRegistry` directly.
#[deprecated(note = "use IpcRegistry")]
pub type CapabilityRegistry = IpcRegistry;
