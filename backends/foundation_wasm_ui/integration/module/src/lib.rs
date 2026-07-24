//! Minimal wasm32 module exercising `foundation_wasm_ui`'s DOM host FFI end-to-end
//! against the real JS runtimes (`foundation-wasm.js` + `foundation-wasm-ui.js`'s
//! `domAbi` import fragment): DOM handle pre-allocation, the naked `as_dom`
//! invocation fast-path, and DOM handle retirement.
//!
//! `std` crate (default allocator + panic handler); the library crates stay `no_std`.

use foundation_ui_traits::DomOp;
use foundation_wasm::abi::web::register_function;
use foundation_wasm::ExternalPointer;
use foundation_wasm_ui::wasm::dom::{allocate_dom_reference, drop_dom_reference, DomInvoke};
use foundation_wasm_ui::{BatchInstructionsV1, ColumnarReceiver, ColumnarV1, InstructionReceiver};

#[no_mangle]
pub extern "C" fn dom_allocate() -> i64 {
    allocate_dom_reference().clone_inner() as i64
}

#[no_mangle]
pub extern "C" fn dom_invoke_for_dom() -> i64 {
    let f = register_function("function(){ return this.testNode; }");
    f.invoke_for_dom(&[]).clone_inner() as i64
}

#[no_mangle]
pub extern "C" fn dom_drop(handle: u64) {
    drop_dom_reference(ExternalPointer::pointer(handle));
}

#[no_mangle]
pub extern "C" fn emit_batch_dom_ops() {
    let mut receiver = InstructionReceiver::with_global_arena(Box::new(BatchInstructionsV1::new()));
    receiver.queue(DomOp::SetText { node_id: 5, text: "batch hi".into() });
    receiver.queue(DomOp::RemoveNode { node_id: 6 });
    receiver.flush();
}

#[no_mangle]
pub extern "C" fn emit_columnar_dom_ops() {
    let mut receiver = ColumnarReceiver::with_global_arena(ColumnarV1::new());
    receiver.queue(&DomOp::SetText { node_id: 5, text: "columnar hi".into() });
    receiver.queue(&DomOp::RemoveNode { node_id: 6 });
    let _ = receiver.flush();
}

// ── F41: Unified IPC e2e exports ────────────────────────────────────────

use foundation_wasm::ipc::{encode_request, decode_response, IpcContentType, IpcRequest};
use foundation_wasm::{TriggerRegistry};
use foundation_wasm::ipc_ffi::IPC_TRIGGER;

#[no_mangle]
pub extern "C" fn e2e_ipc_register_handler(action: u32) -> i32 {
    #[cfg(target_family = "wasm")]
    {
        let mut trigger = IPC_TRIGGER.lock().unwrap();
        trigger.set_ipc_handler(move |req: IpcRequest<Vec<u8>>| {
            use foundation_wasm::ipc::IpcResponse;
            Ok(IpcResponse { payload: format!("handled-{action}-{}", req.action).into_bytes(), content_type: IpcContentType::Json })
        });
    }
    #[cfg(not(target_family = "wasm"))]
    { let _ = action; }
    1
}

#[no_mangle]
pub extern "C" fn e2e_ui_ipc_echo() -> i32 {
    let req = IpcRequest { ipc: "echo_ui".into(), action: "ping".into(), payload: b"hello-ui".to_vec(), content_type: IpcContentType::Json, target: None };
    let encoded = encode_request(&req);
    let alloc_id = unsafe { foundation_wasm::host_ipc::host_ipc_invoke(encoded.as_ptr(), encoded.len() as u32) };
    if alloc_id == 0 { return -3; }
    let resp_bytes = foundation_wasm::internal_api::extract_vec_from_memory(alloc_id);
    foundation_wasm::exposed_runtime::dispose_allocation(alloc_id);
    match decode_response(&resp_bytes) {
        Ok(resp) => if resp.payload == b"hello-ui" { 1 } else { -2 },
        Err(_) => -3,
    }
}

#[no_mangle]
pub extern "C" fn e2e_ui_chrome_set_title() -> i32 {
    let req = IpcRequest { ipc: "chrome".into(), action: "set_title".into(), payload: br#"{"title":"Test Page"}"#.to_vec(), content_type: IpcContentType::Json, target: None };
    let encoded = encode_request(&req);
    let alloc_id = unsafe { foundation_wasm::host_ipc::host_ipc_invoke(encoded.as_ptr(), encoded.len() as u32) };
    if alloc_id == 0 { return -2; }
    let resp_bytes = foundation_wasm::internal_api::extract_vec_from_memory(alloc_id);
    foundation_wasm::exposed_runtime::dispose_allocation(alloc_id);
    match decode_response(&resp_bytes) {
        Ok(_) => 1,
        Err(_) => -2,
    }
}

#[no_mangle]
pub extern "C" fn e2e_ui_trigger_event() -> i32 {
    let req = IpcRequest { ipc: "chrome".into(), action: "toolbar_tap".into(), payload: br#"{"button_id":"save"}"#.to_vec(), content_type: IpcContentType::Json, target: None };
    let encoded = encode_request(&req);
    let alloc_id = foundation_wasm::exposed_runtime::create_allocation(encoded.len() as u64);
    let ptr = foundation_wasm::exposed_runtime::allocation_start_pointer(alloc_id);
    unsafe { core::ptr::copy_nonoverlapping(encoded.as_ptr(), ptr as *mut u8, encoded.len()); }
    let result_alloc = foundation_wasm::host_ipc::ipc_handle_event(ptr, encoded.len() as u32);
    if result_alloc == 0 { return -1; }
    let resp_bytes = foundation_wasm::internal_api::extract_vec_from_memory(result_alloc);
    foundation_wasm::exposed_runtime::dispose_allocation(result_alloc);
    foundation_wasm::exposed_runtime::dispose_allocation(alloc_id);
    match decode_response(&resp_bytes) {
        Ok(resp) => { let s = core::str::from_utf8(&resp.payload).unwrap_or("invalid"); if s.contains("handled") { 1 } else { -2 } }
        Err(_) => -3,
    }
}

// ── F41: IPC Stream exports ─────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn e2e_ui_ipc_stream_open() -> u64 {
    let req = IpcRequest { ipc: "camera".into(), action: "snap".into(), payload: br#"{"format":"jpeg"}"#.to_vec(), content_type: IpcContentType::Json, target: None };
    let encoded = encode_request(&req);
    unsafe { foundation_wasm::host_ipc::host_ipc_stream_open(encoded.as_ptr(), encoded.len() as u32) }
}

#[no_mangle]
pub extern "C" fn e2e_ui_ipc_stream_read(stream_id: u64) -> u64 {
    unsafe { foundation_wasm::host_ipc::host_ipc_stream_read(stream_id) }
}

#[no_mangle]
pub extern "C" fn e2e_ui_ipc_stream_close(stream_id: u64) {
    unsafe { foundation_wasm::host_ipc::host_ipc_stream_close(stream_id) };
}

#[no_mangle]
pub extern "C" fn e2e_ui_ipc_stream_roundtrip() -> i32 {
    let req = IpcRequest { ipc: "camera".into(), action: "snap".into(), payload: br#"{"format":"jpeg"}"#.to_vec(), content_type: IpcContentType::Json, target: None };
    let encoded = encode_request(&req);
    let stream_id = unsafe { foundation_wasm::host_ipc::host_ipc_stream_open(encoded.as_ptr(), encoded.len() as u32) };
    if stream_id == 0 { return -1; }
    let mut chunks = 0;
    loop {
        let alloc_id = unsafe { foundation_wasm::host_ipc::host_ipc_stream_read(stream_id) };
        if alloc_id == 0 { break; }
        let chunk_bytes = foundation_wasm::internal_api::extract_vec_from_memory(alloc_id);
        foundation_wasm::exposed_runtime::dispose_allocation(alloc_id);
        if decode_response(&chunk_bytes).is_err() { return -2; }
        chunks += 1;
    }
    unsafe { foundation_wasm::host_ipc::host_ipc_stream_close(stream_id) };
    if chunks == 3 { 1 } else { -3 }
}
