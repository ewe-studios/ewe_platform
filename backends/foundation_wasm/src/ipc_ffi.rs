//! IPC FFI dispatch.
//!
//! WASM → host: register callback → `host_ipc_invoke(ptr, len, cb_id)`.
//! Host → WASM: `ipc_resolve(cb_id, alloc_id)` fires the callback.

use crate::host_runtime::{exposed_runtime, internal_api};
use crate::{
    ipc::{decode_response, encode_request, IpcError, IpcRequest, IpcResponse, WirePayload},
    MemoryAllocation,
};
use crate::{ReturnTypeHints, ReturnTypeId, ReturnValues, Returns, ThreeState};
use alloc::vec::Vec;

// ── WASM → host ────────────────────────────────────────────────────────

pub fn ipc_invoke(
    wire: IpcRequest<Vec<u8>>,
    callback: impl Fn(Result<IpcResponse<Vec<u8>>, IpcError>) + Send + Sync + 'static,
) -> Result<(), IpcError> {
    let callback_token = internal_api::register_callback(
        ReturnTypeHints::One(ThreeState::Two(
            ReturnTypeId::Uint8ArrayBuffer,
            ReturnTypeId::ErrorCode,
        )),
        move |response_result| match response_result {
            Ok(response) => callback(match response {
                Returns::One(inner) => match inner {
                    ReturnValues::Uint8Array(buffer) => decode_response(buffer.as_slice()),
                    _ => Err(IpcError::InvalidPayload),
                },
                _ => Err(IpcError::InvalidPayload),
            }),
            Err(e) => callback(Err(e.take().into())),
        },
    );

    let serialized = encode_request(&wire);
    let alloc = MemoryAllocation::new(serialized);
    let (ptr, len) = alloc.as_address().map_err(|_| IpcError::ExecutionFailed)?;

    crate::host_runtime::ipc::host_ipc_invoke(ptr, len as u32, callback_token.into());
    Ok(())
}

pub fn ipc_invoke_typed<T: WirePayload, U: WirePayload>(
    _name: &str,
    request: IpcRequest<T>,
    callback: impl Fn(Result<U, IpcError>) + Send + Sync + 'static,
) -> Result<(), IpcError> {
    ipc_invoke(request.into_wire(), move |r| {
        callback(r.and_then(|resp| {
            U::from_wire_bytes(&resp.payload, resp.content_type).map_err(Into::into)
        }));
    })
}

// ── Host → WASM ───────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn ipc_resolve(callback_id: u64, alloc_id: u64) {
    internal_api::run_internal_callbacks(callback_id.into(), alloc_id.into());
    exposed_runtime::dispose_allocation(alloc_id);
    internal_api::unregister_internal_callback(callback_id.into());
}

