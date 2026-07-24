//! Unified IPC — merge of F23 (capabilities) + F25 (IPC). Types only; FFI
//! lives in `host_runtime.rs` alongside the rest of the ABI surface.
//!
//! WHY: IPC and capabilities were identical in shape. One trait, one registry.
//!
//! WHAT: `WirePayload` trait, `IpcContentType`, generic `IpcRequest<T>` /
//! `IpcResponse<T>` with `Vec<u8>` wire defaults, `Ipc<Input, Output>` trait,
//! `IpcRegistry` (Mutex-guarded on native), `IpcKind::Capability` security tag.
//!
//! HOW: `no_std`, `alloc` only. On native: `Send + Sync` + `Mutex`. On wasm32:
//! single-threaded, no bounds.

use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use foundation_nostd::comp::basic::Mutex;

// ── Content type ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcContentType {
    Json,
    Arrow,
    Binary,
}

// ── WirePayload trait ───────────────────────────────────────────────────

pub trait WirePayload: Sized {
    fn into_wire_bytes(self) -> (Vec<u8>, IpcContentType);
    fn from_wire_bytes(data: &[u8], content_type: IpcContentType) -> Result<Self, WireError>;
}

impl WirePayload for Vec<u8> {
    fn into_wire_bytes(self) -> (Vec<u8>, IpcContentType) {
        (self, IpcContentType::Json)
    }
    fn from_wire_bytes(data: &[u8], _ct: IpcContentType) -> Result<Self, WireError> {
        Ok(data.to_vec())
    }
}

#[derive(Debug, Clone)]
pub enum WireError {
    EncodeFailed,
    DecodeFailed,
    UnsupportedContentType,
    UnsupportedOperation,
}

impl From<u16> for WireError {
    fn from(value: u16) -> Self {
        match value {
            5440 => WireError::EncodeFailed,
            5441 => WireError::DecodeFailed,
            5442 => WireError::UnsupportedContentType,
            _ => WireError::UnsupportedOperation,
        }
    }
}

// ── Wire types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct IpcRequest<T = Vec<u8>> {
    pub ipc: String,
    pub action: String,
    pub payload: T,
    pub content_type: IpcContentType,
    pub target: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IpcResponse<T = Vec<u8>> {
    pub payload: T,
    pub content_type: IpcContentType,
}

// wire conversion

impl<T: WirePayload> IpcRequest<T> {
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
    pub fn wire(
        ipc: impl Into<String>,
        action: impl Into<String>,
        payload: T,
    ) -> IpcRequest<Vec<u8>> {
        IpcRequest {
            ipc: ipc.into(),
            action: action.into(),
            payload,
            content_type: IpcContentType::Json,
            target: None,
        }
        .into_wire()
    }
}

impl IpcRequest<Vec<u8>> {
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

impl<T: WirePayload> IpcResponse<T> {
    pub fn into_wire(self) -> IpcResponse<Vec<u8>> {
        let (payload, content_type) = self.payload.into_wire_bytes();
        IpcResponse {
            payload,
            content_type,
        }
    }
    pub fn wire(payload: T) -> IpcResponse<Vec<u8>> {
        IpcResponse {
            payload,
            content_type: IpcContentType::Json,
        }
        .into_wire()
    }
}

impl IpcResponse<Vec<u8>> {
    pub fn into_typed<T: WirePayload>(self) -> Result<IpcResponse<T>, WireError> {
        let payload = T::from_wire_bytes(&self.payload, self.content_type)?;
        Ok(IpcResponse {
            payload,
            content_type: self.content_type,
        })
    }
}

// ── Errors ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum IpcError {
    UnknownIpc,
    InvalidPayload,
    ExecutionFailed,
    PermissionDenied,
    UnsupportedContentType,
}

impl From<u16> for IpcError {
    fn from(value: u16) -> Self {
        match value {
            6441 => IpcError::InvalidPayload,
            6442 => IpcError::ExecutionFailed,
            6443 => IpcError::PermissionDenied,
            6444 => IpcError::UnsupportedContentType,
            _ => IpcError::UnknownIpc,
        }
    }
}

impl From<WireError> for IpcError {
    fn from(e: WireError) -> Self {
        match e {
            WireError::EncodeFailed | WireError::DecodeFailed => IpcError::InvalidPayload,
            WireError::UnsupportedContentType => IpcError::UnsupportedContentType,
            WireError::UnsupportedOperation => IpcError::ExecutionFailed,
        }
    }
}

// ── IpcKind ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcKind {
    Query,
    Emit,
    Page,
    Capability,
}

// ── Ipc trait ───────────────────────────────────────────────────────────

#[cfg(target_family = "wasm")]
pub trait Ipc<Input = Vec<u8>, Output = Vec<u8>> {
    fn name(&self) -> &str;
    fn kind(&self) -> IpcKind;
    fn invoke(&self, request: &IpcRequest<Input>) -> Result<IpcResponse<Output>, IpcError>;
}

#[cfg(not(target_family = "wasm"))]
pub trait Ipc<Input = Vec<u8>, Output = Vec<u8>>: Send + Sync {
    fn name(&self) -> &str;
    fn kind(&self) -> IpcKind;
    fn invoke(&self, request: &IpcRequest<Input>) -> Result<IpcResponse<Output>, IpcError>;
}

// ── Registry ────────────────────────────────────────────────────────────

#[cfg(target_family = "wasm")]
type DynIpc = Box<dyn Ipc<Vec<u8>, Vec<u8>> + 'static>;

#[cfg(not(target_family = "wasm"))]
type DynIpc = Box<dyn Ipc<Vec<u8>, Vec<u8>> + Send + Sync + 'static>;

pub struct IpcRegistry {
    #[cfg(not(target_family = "wasm"))]
    inner: Mutex<BTreeMap<String, DynIpc>>,
    #[cfg(target_family = "wasm")]
    inner: BTreeMap<String, DynIpc>,
}

impl IpcRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            #[cfg(not(target_family = "wasm"))]
            inner: Mutex::new(BTreeMap::new()),
            #[cfg(target_family = "wasm")]
            inner: BTreeMap::new(),
        }
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn register(&self, ipc: impl Ipc<Vec<u8>, Vec<u8>> + 'static) -> Option<DynIpc> {
        self.inner
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .insert(String::from(ipc.name()), Box::new(ipc))
    }

    #[cfg(target_family = "wasm")]
    pub fn register(&mut self, ipc: impl Ipc<Vec<u8>, Vec<u8>> + 'static) -> Option<DynIpc> {
        self.inner.insert(String::from(ipc.name()), Box::new(ipc))
    }

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

    pub fn invoke(&self, request: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        match self.get(&request.ipc) {
            Some(ipc) => ipc.invoke(request),
            None => Err(IpcError::UnknownIpc),
        }
    }

    pub fn invoke_typed<T: WirePayload, U: WirePayload>(
        &self,
        request: IpcRequest<T>,
    ) -> Result<IpcResponse<U>, IpcError> {
        Ok(self.invoke(&request.into_wire())?.into_typed()?)
    }

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

// ── Binary encoding (ToBinary / FromBinary for IpcRequest / IpcResponse) ──

use crate::{BinaryReadError, FromBinary, ToBinary};

/// Wire format: length-delimited binary (same pattern as Params).
///   4 bytes LE u32: ipc name length + ipc name bytes
///   4 bytes LE u32: action length + action bytes
///   1 byte:         content_type (0=Json, 1=Arrow, 2=Binary)
///   4 bytes LE u32: target length + target bytes (0 length = None)
///   4 bytes LE u32: payload length + payload bytes
fn write_str(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
}

fn read_str(data: &[u8], off: &mut usize) -> Result<String, BinaryReadError> {
    if *off + 4 > data.len() {
        return Err(BinaryReadError::MemoryError("truncated".into()));
    }
    let len =
        u32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]) as usize;
    *off += 4;
    if *off + len > data.len() {
        return Err(BinaryReadError::MemoryError("truncated".into()));
    }
    let s = String::from_utf8(data[*off..*off + len].to_vec())
        .map_err(|e| BinaryReadError::MemoryError(alloc::format!("utf8: {e}")))?;
    *off += len;
    Ok(s)
}

fn ct_byte(ct: IpcContentType) -> u8 {
    match ct {
        IpcContentType::Json => 0,
        IpcContentType::Arrow => 1,
        IpcContentType::Binary => 2,
    }
}

fn ct_from_byte(b: u8) -> Result<IpcContentType, BinaryReadError> {
    match b {
        0 => Ok(IpcContentType::Json),
        1 => Ok(IpcContentType::Arrow),
        2 => Ok(IpcContentType::Binary),
        _ => Err(BinaryReadError::MemoryError(alloc::format!(
            "unknown content type: {b}"
        ))),
    }
}

// ── Module-level encode/decode (primary FFI API) ────────────────────────

/// Encode an `IpcRequest<Vec<u8>>` into wire bytes.
pub fn encode_request(req: &IpcRequest<Vec<u8>>) -> Vec<u8> {
    let mut buf = Vec::new();
    write_str(&mut buf, &req.ipc);
    write_str(&mut buf, &req.action);
    buf.push(ct_byte(req.content_type));
    let target = req.target.as_deref().unwrap_or("");
    write_str(&mut buf, target);
    buf.extend_from_slice(&(req.payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(&req.payload);
    buf
}

/// Encode an `IpcResponse<Vec<u8>>` into wire bytes (compact format).
///   1 byte:  content_type (0=Json, 1=Arrow, 2=Binary)
///   4 bytes LE u32: payload length
///   N bytes: payload
pub fn encode_response(resp: &IpcResponse<Vec<u8>>) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5 + resp.payload.len());
    buf.push(ct_byte(resp.content_type));
    buf.extend_from_slice(&(resp.payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(&resp.payload);
    buf
}

/// Decode an `IpcResponse<Vec<u8>>` from wire bytes (compact format).
pub fn decode_response(data: &[u8]) -> Result<IpcResponse<Vec<u8>>, IpcError> {
    if data.len() < 5 {
        return Err(IpcError::InvalidPayload);
    }
    let ct = ct_from_byte(data[0]).map_err(|_e| IpcError::InvalidPayload)?;
    let plen = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;
    if data.len() < 5 + plen {
        return Err(IpcError::InvalidPayload);
    }
    Ok(IpcResponse {
        payload: data[5..5 + plen].to_vec(),
        content_type: ct,
    })
}

/// Decode an `IpcRequest<Vec<u8>>` from wire bytes.
pub fn decode_request(data: &[u8]) -> Result<IpcRequest<Vec<u8>>, IpcError> {
    let mut off = 0;
    let ipc = read_str(data, &mut off).map_err(|_e| IpcError::InvalidPayload)?;
    let action = read_str(data, &mut off).map_err(|_e| IpcError::InvalidPayload)?;
    let ct =
        ct_from_byte(data.get(off).copied().unwrap_or(0)).map_err(|_e| IpcError::InvalidPayload)?;
    off += 1;
    let target_s = read_str(data, &mut off).map_err(|_e| IpcError::InvalidPayload)?;
    let target = if target_s.is_empty() {
        None
    } else {
        Some(target_s)
    };
    if off + 4 > data.len() {
        return Err(IpcError::InvalidPayload);
    }
    let plen =
        u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]) as usize;
    off += 4;
    if off + plen > data.len() {
        return Err(IpcError::InvalidPayload);
    }
    Ok(IpcRequest {
        ipc,
        action,
        payload: data[off..off + plen].to_vec(),
        content_type: ct,
        target,
    })
}

// ── ToBinary / FromBinary impls (delegate to module functions) ──────────

impl ToBinary for IpcRequest<Vec<u8>> {
    fn to_binary(&self) -> Vec<u8> {
        encode_request(self)
    }
}

impl ToBinary for IpcResponse<Vec<u8>> {
    fn to_binary(&self) -> Vec<u8> {
        encode_response(self)
    }
}

impl FromBinary for IpcRequest<Vec<u8>> {
    type T = IpcRequest<Vec<u8>>;
    fn from_binary(self, data: &[u8]) -> crate::BinaryReaderResult<Self::T> {
        decode_request(data).map_err(|e| BinaryReadError::MemoryError(alloc::format!("{e:?}")))
    }
}

impl FromBinary for IpcResponse<Vec<u8>> {
    type T = IpcResponse<Vec<u8>>;
    fn from_binary(self, data: &[u8]) -> crate::BinaryReaderResult<Self::T> {
        decode_response(data).map_err(|e| BinaryReadError::MemoryError(alloc::format!("{e:?}")))
    }
}

// ── Name validation ─────────────────────────────────────────────────────

#[must_use]
pub fn is_valid_ipc_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}
