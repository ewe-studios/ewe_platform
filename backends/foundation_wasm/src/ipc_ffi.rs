//! IPC FFI dispatch helpers (F41 Part B + F43 async).
//!
//! Two dispatch paths:
//! 1. `ipc_dispatch` (sync, legacy): blocks WASM on `host_ipc_invoke` — only works
//!    when the host has a synchronous handler (browser/Deno with wasm-bindgen,
//!    or tests). Does NOT work on Tauri (async invoke).
//! 2. `ipc_dispatch_async` (F43): serializes request, registers callback in global
//!    registry keyed by token, calls `host_ipc_invoke_async(ptr, len, token)`.
//!    The host calls `ipc_resolve(token, resp_ptr, resp_len)` when the response
//!    is ready. Works on all hosts (Tauri, browser, Deno).

use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::vec::Vec;

use foundation_nostd::comp::basic::Mutex;

use crate::ipc::{
    IpcError, IpcRequest, IpcResponse, WirePayload,
    encode_request, encode_response, decode_response, decode_request,
};
use crate::{MemoryAllocation, ToBinary, TriggerRegistry};

// ── Token registry for async callbacks (F43) ────────────────────────────

static PENDING: Mutex<Option<BTreeMap<u64, Box<dyn FnOnce(Result<IpcResponse<Vec<u8>>, IpcError>) + Send>>>> = Mutex::new(None);
static NEXT_TOKEN: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);

fn push_callback(
    f: Box<dyn FnOnce(Result<IpcResponse<Vec<u8>>, IpcError>) + Send>,
) -> u64 {
    let token = NEXT_TOKEN.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    let mut guard = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() { *guard = Some(BTreeMap::new()); }
    guard.as_mut().unwrap().insert(token, f);
    token
}

#[cfg(target_family = "wasm")]
fn take_callback(token: u64) -> Option<Box<dyn FnOnce(Result<IpcResponse<Vec<u8>>, IpcError>) + Send>> {
    let mut guard = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_mut().and_then(|m| m.remove(&token))
}

// ── dispatch_async (F43) — works on Tauri ──────────────────────────

#[cfg(target_family = "wasm")]
pub fn ipc_dispatch_async(
    name: &str,
    request: IpcRequest<Vec<u8>>,
    callback: Box<dyn FnOnce(Result<IpcResponse<Vec<u8>>, IpcError>) + Send>,
) -> Result<(), IpcError> {
    use crate::host_runtime::ipc::host_ipc_invoke_async;

    let serialized = encode_request(&request);
    let alloc = MemoryAllocation::new(serialized);
    let (ptr, len) = alloc.as_address()
        .map_err(|e| IpcError::ExecutionFailed(alloc::format!("memory: {e:?}")))?;

    let token = push_callback(callback);
    unsafe { host_ipc_invoke_async(ptr, len as u32, token); }
    Ok(())
}

/// Called by the host (JS) when the async IPC response is ready.
/// Deserializes the response, fires the callback, disposes the allocation.
#[cfg(target_family = "wasm")]
#[no_mangle]
pub extern "C" fn ipc_resolve(token: u64, resp_alloc_id: u64) {
    use crate::host_runtime::{exposed_runtime, internal_api};

    let resp_bytes = internal_api::extract_vec_from_memory(resp_alloc_id);
    exposed_runtime::dispose_allocation(resp_alloc_id);

    let result = decode_response(&resp_bytes);

    if let Some(cb) = take_callback(token) {
        cb(result);
    }
}

// ── dispatch (sync, legacy) ─────────────────────────────────────────────

/// Serialize → allocate → host_ipc_invoke → extract → deserialize.
/// Only works with synchronous host handlers (not Tauri).
#[cfg(target_family = "wasm")]
pub fn ipc_dispatch<T: WirePayload, U: WirePayload>(
    name: &str,
    request: IpcRequest<T>,
) -> Result<IpcResponse<U>, IpcError> {
    use crate::host_runtime::internal_api;

    let IpcRequest { ipc: _, action, payload, content_type, target } = request;
    let (payload_bytes, payload_ct) = payload.into_wire_bytes();
    let ct = if payload_ct != crate::ipc::IpcContentType::Json { payload_ct } else { content_type };

    let wire_req = IpcRequest {
        ipc: name.into(),
        action,
        payload: payload_bytes,
        content_type: ct,
        target,
    };

    let serialized = encode_request(&wire_req);

    let alloc = MemoryAllocation::new(serialized);
    let (ptr, len) = alloc.as_address()
        .map_err(|e| IpcError::ExecutionFailed(alloc::format!("memory: {e:?}")))?;

    let resp_alloc_id = unsafe {
        crate::host_runtime::ipc::host_ipc_invoke(ptr, len as u32)
    };
    let resp_bytes = internal_api::extract_vec_from_memory(resp_alloc_id);
    crate::host_runtime::exposed_runtime::dispose_allocation(resp_alloc_id);

    let resp_wire = decode_response(&resp_bytes)?;
    let output = U::from_wire_bytes(&resp_wire.payload, resp_wire.content_type)?;

    Ok(IpcResponse { payload: output, content_type: resp_wire.content_type })
}

// ── Trigger registry (host → WASM events) ───────────────────────────────

pub static IPC_TRIGGER: Mutex<TriggerRegistry> = Mutex::new(TriggerRegistry::new());

#[cfg(not(target_family = "wasm"))]
pub fn set_event_handler(
    handler: impl Fn(IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> + Send + Sync + 'static,
) {
    #[allow(clippy::unwrap_used)]
    IPC_TRIGGER.lock().unwrap().set_ipc_handler(handler);
}

#[cfg(target_family = "wasm")]
pub fn set_event_handler(
    handler: impl Fn(IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> + 'static,
) {
    #[allow(clippy::unwrap_used)]
    IPC_TRIGGER.lock().unwrap().set_ipc_handler(handler);
}

/// Called by `host_runtime::ipc::ipc_handle_event`.
#[cfg(target_family = "wasm")]
pub fn handle_event(event_ptr: *const u8, event_len: u32) -> u64 {
    use crate::host_runtime::exposed_runtime;

    let data = unsafe { core::slice::from_raw_parts(event_ptr, event_len as usize) };
    let request = match decode_request(data) {
        Ok(req) => req,
        Err(_) => return 0,
    };
    #[allow(clippy::unwrap_used)]
    let trigger = IPC_TRIGGER.lock().unwrap();
    let response = match trigger.dispatch_ipc(request) {
        Ok(r) => r,
        Err(_) => return 0,
    };
    let serialized = encode_response(&response);
    let alloc_id = exposed_runtime::create_allocation(serialized.len() as u64);
    let ptr = exposed_runtime::allocation_start_pointer(alloc_id);
    unsafe { core::ptr::copy_nonoverlapping(serialized.as_ptr(), ptr as *mut u8, serialized.len()) };
    alloc_id
}

#[cfg(not(target_family = "wasm"))]
pub fn handle_event(_event_ptr: *const u8, _event_len: u32) -> u64 { 0 }
