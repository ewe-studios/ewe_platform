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

/// Pre-allocate a DOM-heap slot (`dom_allocate_external_pointer`) → its handle.
/// Slots 0–4 are reserved (self/heap/window/document/body), so the first
/// pre-allocation returns index 5.
#[no_mangle]
pub extern "C" fn dom_allocate() -> i64 {
    allocate_dom_reference().clone_inner() as i64
}

/// NAKED dom fast-path: `invoke_for_dom` rides `host_invoke_function_as_dom` — the
/// JS fn's returned node interns into the DOM heap and the HANDLE crosses raw.
/// The test puts the node on the registry context as `this.testNode`.
#[no_mangle]
pub extern "C" fn dom_invoke_for_dom() -> i64 {
    let f = register_function("function(){ return this.testNode; }");
    f.invoke_for_dom(&[]).clone_inner() as i64
}

/// Retires a DOM-heap handle (`host_dom_drop_external_pointer` round-trip).
#[no_mangle]
pub extern "C" fn dom_drop(handle: u64) {
    drop_dom_reference(ExternalPointer::pointer(handle));
}

/// Ship a DomOp batch over the CUSTOM BINARY protocol (byte 0 = the batch
/// instructions format, decision 022) through the LIVE LOOP: a global-arena
/// InstructionReceiver (decision 030) flushes once, BatchInstructionsV1 packs the
/// Operations stream + texts pool into one envelope slot, and host_apply ships it —
/// the JS dispatcher routes byte 0 into the BatchInstructions runtime, where the
/// registered BATCH_OP_APPLY_DOM operation applies each op to the DOM.
#[no_mangle]
pub extern "C" fn emit_batch_dom_ops() {
    let mut receiver = InstructionReceiver::with_global_arena(Box::new(BatchInstructionsV1::new()));
    receiver.queue(DomOp::SetText {
        node_id: 5,
        text: "batch hi".into(),
    });
    receiver.queue(DomOp::RemoveNode { node_id: 6 });
    receiver.flush();
}

/// Ship a DomOp batch over the COLUMNAR protocol (byte 1, wire v1.1) through
/// the columnar-native path: ColumnarReceiver accumulates straight into column
/// buffers (no Vec<DomOp>), and ColumnarV1's framing computes the alignment
/// shim against the ABSOLUTE arena address — the JS ColumnarParser must get
/// TRUE zero-copy TypedArray views (feature 19).
#[no_mangle]
pub extern "C" fn emit_columnar_dom_ops() {
    let mut receiver = ColumnarReceiver::with_global_arena(ColumnarV1::new());
    receiver.queue(&DomOp::SetText {
        node_id: 5,
        text: "columnar hi".into(),
    });
    receiver.queue(&DomOp::RemoveNode { node_id: 6 });
    let _ = receiver.flush();
}


// ── F41: Unified IPC e2e exports (WASM→host + host→WASM) ────────────────

use foundation_wasm::ipc::{encode_request, decode_response, IpcContentType, IpcRequest};
use foundation_wasm::{TriggerRegistry, MemoryAllocation};
use foundation_wasm::ipc_ffi::IPC_TRIGGER;

/// Register a mock IPC handler in the global trigger registry.
/// The JS test calls this, then triggers host→WASM events.
#[no_mangle]
pub extern "C" fn e2e_ipc_register_handler(action: u32) -> i32 {
    #[cfg(target_family = "wasm")]
    {
        let mut trigger = IPC_TRIGGER.lock().unwrap();
        trigger.set_ipc_handler(move |req: IpcRequest<Vec<u8>>| {
            use foundation_wasm::ipc::IpcResponse;
            Ok(IpcResponse {
                payload: format!("handled-{action}-{}", req.action).into_bytes(),
                content_type: IpcContentType::Json,
            })
        });
    }
    #[cfg(not(target_family = "wasm"))]
    { let _ = action; }
    1
}

/// Call host_ipc_invoke from WASM with a binary-encoded echo request.
/// Returns 1 if the host echoes the payload back, -1 on failure.
#[no_mangle]
pub extern "C" fn e2e_ui_ipc_echo() -> i32 {
    let req = IpcRequest {
        ipc: "echo_ui".into(),
        action: "ping".into(),
        payload: b"hello-ui".to_vec(),
        content_type: IpcContentType::Json,
        target: None,
    };
    let encoded = encode_request(&req);
    let alloc_id = unsafe {
        foundation_wasm::host_ipc::host_ipc_invoke(encoded.as_ptr(), encoded.len() as u32)
    };
    let resp_bytes = foundation_wasm::internal_api::extract_vec_from_memory(alloc_id);
    foundation_wasm::exposed_runtime::dispose_allocation(alloc_id);

    match decode_response(&resp_bytes) {
        Ok(resp) => if resp.payload == b"hello-ui" { 1 } else { -2 },
        Err(_) => -3,
    }
}

/// Call host_ipc_invoke for a "chrome:set_title" capability.
#[no_mangle]
pub extern "C" fn e2e_ui_chrome_set_title() -> i32 {
    let req = IpcRequest {
        ipc: "chrome".into(),
        action: "set_title".into(),
        payload: br#"{"title":"Test Page"}"#.to_vec(),
        content_type: IpcContentType::Json,
        target: None,
    };
    let encoded = encode_request(&req);
    let alloc_id = unsafe {
        foundation_wasm::host_ipc::host_ipc_invoke(encoded.as_ptr(), encoded.len() as u32)
    };
    let resp_bytes = foundation_wasm::internal_api::extract_vec_from_memory(alloc_id);
    foundation_wasm::exposed_runtime::dispose_allocation(alloc_id);

    match decode_response(&resp_bytes) {
        Ok(_) => 1,
        Err(_) => -2,
    }
}

/// Trigger a host→WASM event by encoding a request and writing it
/// to memory, then calling ipc_handle_event export (the entrypoint the
/// JS host calls). Returns the allocation ID for JS to inspect.
#[no_mangle]
pub extern "C" fn e2e_ui_trigger_event() -> i32 {
    let req = IpcRequest {
        ipc: "chrome".into(),
        action: "toolbar_tap".into(),
        payload: br#"{"button_id":"save"}"#.to_vec(),
        content_type: IpcContentType::Json,
        target: None,
    };
    let encoded = encode_request(&req);

    // Write event bytes into memory and call ipc_handle_event
    let alloc_id = foundation_wasm::exposed_runtime::create_allocation(encoded.len() as u64);
    let ptr = foundation_wasm::exposed_runtime::allocation_start_pointer(alloc_id);
    unsafe { core::ptr::copy_nonoverlapping(encoded.as_ptr(), ptr as *mut u8, encoded.len()); }

    let result_alloc = foundation_wasm::host_ipc::ipc_handle_event(ptr, encoded.len() as u32);
    if result_alloc == 0 { return -1; }

    let resp_bytes = foundation_wasm::internal_api::extract_vec_from_memory(result_alloc);
    foundation_wasm::exposed_runtime::dispose_allocation(result_alloc);
    foundation_wasm::exposed_runtime::dispose_allocation(alloc_id);

    match decode_response(&resp_bytes) {
        Ok(resp) => {
            let s = core::str::from_utf8(&resp.payload).unwrap_or("invalid");
            if s.contains("handled") { 1 } else { -2 }
        }
        Err(_) => -3,
    }
}
