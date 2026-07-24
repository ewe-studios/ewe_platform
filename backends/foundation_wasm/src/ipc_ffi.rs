//! IPC FFI dispatch helpers (F41 Part B).
//!
//! Host imports live in `host_runtime::ipc`, the `ipc_handle_event` export
//! in `host_runtime::ipc`. This module provides the typed dispatch wrapper
//! using the same `MemoryAllocation` → `as_address()` → host import →
//! `extract_vec_from_memory` pattern as `host_runtime::abi::web::batch()`.
//!
//! Encoding/decoding delegated to `crate::ipc::{encode_request,decode_response,decode_request}`.

use alloc::vec::Vec;

use foundation_nostd::comp::basic::Mutex;

use crate::ipc::{
    IpcError, IpcRequest, IpcResponse, WirePayload,
    encode_request, encode_response, decode_response, decode_request,
};
use crate::{MemoryAllocation, ToBinary};
use crate::TriggerRegistry;

// ── Dispatch (WASM → host) ─────────────────────────────────────────────

/// Serialize → allocate → host_ipc_invoke → extract → deserialize.
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

    // 1. Serialize via module function
    let serialized = encode_request(&wire_req);

    // 2. Wrap in MemoryAllocation, get (ptr, len)
    let alloc = MemoryAllocation::new(serialized);
    let (ptr, len) = alloc.as_address()
        .map_err(|e| IpcError::ExecutionFailed(alloc::format!("memory: {e:?}")))?;

    // 3. Call host import
    let resp_alloc_id = unsafe {
        crate::host_runtime::ipc::host_ipc_invoke(ptr, len as u32)
    };
    if resp_alloc_id == 0 {
        return Err(IpcError::ExecutionFailed(alloc::format!("host_ipc_invoke returned 0 for '{name}'")));
    }

    // 4. Extract + dispose
    let resp_bytes = internal_api::extract_vec_from_memory(resp_alloc_id);
    crate::host_runtime::exposed_runtime::dispose_allocation(resp_alloc_id);

    // 5. Decode via module function
    let resp_wire = decode_response(&resp_bytes)?;
    let output = U::from_wire_bytes(&resp_wire.payload, resp_wire.content_type)?;

    Ok(IpcResponse { payload: output, content_type: resp_wire.content_type })
}

// ── Global trigger for host → WASM events ──────────────────────────────

pub static IPC_TRIGGER: Mutex<TriggerRegistry> = Mutex::new(TriggerRegistry::new());

pub fn set_event_handler(
    handler: impl Fn(IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> + Send + Sync + 'static,
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
