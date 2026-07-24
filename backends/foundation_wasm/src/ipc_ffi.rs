//! IPC FFI dispatch helpers (F41 Part B).
//!
//! The host imports live in `host_runtime::abi::web`, the `ipc_handle_event`
//! export in `host_runtime::exposed_runtime`. This module provides the typed
//! dispatch wrapper using the same `MemoryAllocation` → `as_address()` →
//! host import → `extract_vec_from_memory` pattern as `host_runtime::abi::web::batch()`.
//!
//! Wire format: `IpcRequest<Vec<u8>>` serializes via `ToBinary::to_binary()`,
//! `IpcResponse<Vec<u8>>` deserializes via `FromBinary::from_binary()`.

use alloc::string::String;
use alloc::vec::Vec;

use foundation_nostd::comp::basic::Mutex;

use crate::ipc::{IpcError, IpcRequest, IpcResponse, WirePayload};
use crate::{FromBinary, MemoryAllocation, ToBinary};
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
    let ct = if payload_ct != IpcContentType::Json { payload_ct } else { content_type };

    // 1. Build the wire-format request and serialize via ToBinary
    let wire_req = IpcRequest {
        ipc: name.into(),
        action,
        payload: payload_bytes,
        content_type: ct,
        target,
    };
    let serialized = wire_req.to_binary();

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

    // 4. Extract response bytes from the arena
    let resp_bytes = internal_api::extract_vec_from_memory(resp_alloc_id);

    // 5. Dispose the response allocation
    crate::host_runtime::exposed_runtime::dispose_allocation(resp_alloc_id);

    // 6. Decode via FromBinary
    let dummy_resp = IpcResponse { payload: Vec::new(), content_type: IpcContentType::Json };
    let resp_wire = dummy_resp.from_binary(&resp_bytes)
        .map_err(|e| IpcError::InvalidPayload(alloc::format!("decode: {e:?}")))?;
    let (resp_ct, resp_payload) = (resp_wire.content_type, resp_wire.payload);
    let output = U::from_wire_bytes(&resp_payload, resp_ct)?;

    Ok(IpcResponse { payload: output, content_type: resp_ct })
}

// ── Global trigger for host → WASM events ──────────────────────────────

pub static IPC_TRIGGER: Mutex<TriggerRegistry> = Mutex::new(TriggerRegistry::new());

pub fn set_event_handler(
    handler: impl Fn(IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> + Send + Sync + 'static,
) {
    #[allow(clippy::unwrap_used)]
    IPC_TRIGGER.lock().unwrap().set_ipc_handler(handler);
}

/// Called by `host_runtime::exposed_runtime::ipc_handle_event`.
/// Decodes the event, dispatches through IPC_TRIGGER, allocates response.
#[cfg(target_family = "wasm")]
pub fn handle_event(event_ptr: *const u8, event_len: u32) -> u64 {
    use crate::host_runtime::exposed_runtime;

    let data = unsafe { core::slice::from_raw_parts(event_ptr, event_len as usize) };

    // Decode request via FromBinary
    let dummy_req = IpcRequest { ipc: String::new(), action: String::new(), payload: Vec::new(), content_type: IpcContentType::Json, target: None };
    let request = match dummy_req.from_binary(data) {
        Ok(req) => req,
        Err(_) => return 0,
    };

    // Dispatch through trigger
    #[allow(clippy::unwrap_used)]
    let trigger = IPC_TRIGGER.lock().unwrap();
    let response = match trigger.dispatch_ipc(request) {
        Ok(r) => r,
        Err(_) => return 0,
    };

    // Serialize response via ToBinary
    let serialized = response.to_binary();

    // Allocate and write result
    let alloc_id = exposed_runtime::create_allocation(serialized.len() as u64);
    let ptr = exposed_runtime::allocation_start_pointer(alloc_id);
    unsafe { core::ptr::copy_nonoverlapping(serialized.as_ptr(), ptr as *mut u8, serialized.len()) };
    alloc_id
}

#[cfg(not(target_family = "wasm"))]
pub fn handle_event(_event_ptr: *const u8, _event_len: u32) -> u64 { 0 }
