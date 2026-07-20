//! Minimal wasm32 module that exercises the real foundation_wasm ABI so
//! `foundation-wasm.js` can be tested end-to-end against an actual instance.
//!
//! `emit_columnar_batch` allocates a slot in the global arena, frames an Arrow-encoded
//! `DomOp` batch in a `WasmEnvelope`, and ships it through the uniform `host_apply`
//! import — the exact WASM→JS transport path the JS dispatcher consumes.
//!
//! This is a `std` crate (like the other nodejs integration fixtures) so it links the
//! default allocator + panic handler; `foundation_wasm` itself stays `no_std`.

use foundation_ui_traits::{ColumnarEncoder, DomOp, ProtocolEncoder};
use foundation_wasm::abi::web::{
    allocate_function_reference, batch, batch_response, cache_text, drop_cached_string,
    drop_object_reference, host_apply, invoke_as_bool, invoke_as_f64, invoke_as_i32,
    register_function,
};
use foundation_wasm::{
    exposed_runtime, internal_api, ExternalPointer, InternalPointer, MemoryId, Params,
    ReturnTypeHints, ReturnTypeId, ReturnValues, Returns, ThreeState, WasmEnvelope,
};

/// Build a 2-op compact-columnar batch and ship it to JS via `host_apply`.
///
/// # Panics
/// Panics if the arena slot can't be addressed (never in practice).
#[no_mangle]
pub extern "C" fn emit_columnar_batch() {
    let ops = vec![
        DomOp::SetText {
            node_id: 5,
            text: "hi".into(),
        },
        DomOp::RemoveNode { node_id: 6 },
    ];
    let payload = ColumnarEncoder.encode(ops);

    // Allocate in the GLOBAL arena (the one JS's dispose_allocation frees).
    let total = (WasmEnvelope::HEADER_LEN + payload.len()) as u64;
    let mem_id = exposed_runtime::create_allocation(total);

    // Frame with the real memory id and write it into the slot.
    let framed = WasmEnvelope::write(1, 0, mem_id, &payload);
    let slot = internal_api::get_memory(MemoryId::from_u64(mem_id));
    slot.apply(|m| {
        m.clear();
        m.extend_from_slice(&framed);
    });

    let (ptr, len) = slot.as_address().expect("slot address");
    unsafe { host_apply(mem_id, ptr as u64, len) };
}

// ── Function-call ABI round-trips (validate the FunctionRegistry codec) ──────────
// Each registers a JS fn (source string), invokes it with real `Params` (flat encoding),
// and returns the typed result so the JS test can assert the full Rust↔JS round-trip.

/// `x * 2` via Int32 param + i32 return (typed fast-path, naked).
#[no_mangle]
pub extern "C" fn roundtrip_i32(input: i32) -> i32 {
    let f = register_function("function(x){ return x * 2; }");
    invoke_as_i32(f.handler, &[Params::Int32(input)])
}

/// `x + 0.5` via Float64 param + f64 return.
#[no_mangle]
pub extern "C" fn roundtrip_f64(input: f64) -> f64 {
    let f = register_function("function(x){ return x + 0.5; }");
    invoke_as_f64(f.handler, &[Params::Float64(input)])
}

/// `a && b` via two Bool params + bool return (returned as i32 for the FFI boundary).
#[no_mangle]
pub extern "C" fn roundtrip_bool_and(a: i32, b: i32) -> i32 {
    let f = register_function("function(a, b){ return a && b; }");
    i32::from(invoke_as_bool(f.handler, &[Params::Bool(a != 0), Params::Bool(b != 0)]))
}

/// TRUE generic-encoded path: `invoke_for_replies` forces host_invoke_function →
/// JS encode_into_memory (Begin..End framed) → MemoryId → Rust `from_binary` decode.
/// Validates the marker-wrapped slot-based ReturnValues encode/decode parity.
#[no_mangle]
pub extern "C" fn roundtrip_via_reply_i32(input: i32) -> i32 {
    let f = register_function("function(x){ return x + 7; }");
    match f.invoke_for_replies(
        &[Params::Int32(input)],
        ReturnTypeHints::One(ThreeState::One(ReturnTypeId::Int32)),
    ) {
        Ok(values) => match values.into_iter().next() {
            Some(ReturnValues::Int32(v)) => v,
            _ => -1,
        },
        Err(_) => -2,
    }
}

/// STRING return: the JS fn returns a string; ReplyEncoder writes it to a slot and sends
/// [Text8][slot_id]; Rust `invoke_for_str` decodes it back. Returns the length to assert.
#[no_mangle]
pub extern "C" fn roundtrip_string_len() -> i32 {
    let f = register_function("function(){ return 'hello world'; }");
    f.invoke_for_str(&[]).map(|s| s.len() as i32).unwrap_or(-1)
}

/// STRING in (Text8 param) + STRING out: `s + s`, returns the result length.
#[no_mangle]
pub extern "C" fn roundtrip_string_echo_len() -> i32 {
    let f = register_function("function(s){ return s + s; }");
    f.invoke_for_str(&[Params::Text8("ab")])
        .map(|s| s.len() as i32)
        .unwrap_or(-1)
}

/// LIST return: JS returns `[1, 2, 3]`; a `List(One(Int32))` hint decodes a homogeneous
/// `Vec<ReturnValues::Int32>`. Returns the sum (6) to assert the multi-value frame.
#[no_mangle]
pub extern "C" fn roundtrip_list_sum() -> i32 {
    let f = register_function("function(){ return [1, 2, 3]; }");
    match f.invoke_for_replies(&[], ReturnTypeHints::List(ThreeState::One(ReturnTypeId::Int32))) {
        Ok(values) => values
            .into_iter()
            .map(|v| match v {
                ReturnValues::Int32(n) => n,
                _ => 0,
            })
            .sum(),
        Err(_) => -1,
    }
}

/// MULTI return: JS returns the tuple `[7, true]`; a `Multi[One(Int32), One(Bool)]` hint
/// decodes `(Int32, Bool)` (value k ↔ states[k]). Returns the int when the bool is true.
#[no_mangle]
pub extern "C" fn roundtrip_multi() -> i32 {
    let f = register_function("function(){ return [7, true]; }");
    match f.invoke_for_replies(
        &[],
        ReturnTypeHints::Multi(vec![
            ThreeState::One(ReturnTypeId::Int32),
            ThreeState::One(ReturnTypeId::Bool),
        ]),
    ) {
        Ok(values) => {
            let mut it = values.into_iter();
            let n = match it.next() {
                Some(ReturnValues::Int32(n)) => n,
                _ => return -1,
            };
            match it.next() {
                Some(ReturnValues::Bool(true)) => n,
                _ => -1,
            }
        }
        Err(_) => -2,
    }
}

/// ASYNC return: registers an `async` JS fn (`x + 1`) and invokes it through the async
/// ABI with a caller-supplied callback id. JS resolves the Promise, frames the reply
/// (`[Begin][Int32][value][End]`), and calls `invoke_callback(callback_id, mem)` — the
/// JS test asserts that delivery by spying on the runtime's callback registry.
#[no_mangle]
pub extern "C" fn invoke_async_test(callback_id: u64) {
    let f = register_function("async function(x){ return x + 1; }");
    f.invoke_async(
        InternalPointer::pointer(callback_id),
        &[Params::Int32(41)],
        ReturnTypeHints::One(ThreeState::One(ReturnTypeId::Int32)),
    );
}

/// OBJECT return: the JS fn returns a plain object; ReplyEncoder interns it in the
/// host object heap and sends `[Object=28][handle:u64]`. Returns the handle so the JS
/// test can resolve it back out of `rt.objects`.
#[no_mangle]
pub extern "C" fn roundtrip_object_handle() -> i64 {
    let f = register_function("function(){ return { a: 1 }; }");
    match f.invoke_for_replies(
        &[],
        ReturnTypeHints::One(ThreeState::One(ReturnTypeId::Object)),
    ) {
        Ok(values) => match values.into_iter().next() {
            Some(ReturnValues::Object(ptr)) => ptr.clone_inner() as i64,
            _ => -1,
        },
        Err(_) => -2,
    }
}

/// TYPED-SLICE return: the JS fn returns a Uint8Array; ReplyEncoder stages the bytes in
/// a slot and sends `[TypedArraySlice=32][slice_type][ptr:u64][len:u64]`. Rust reads the
/// raw MemoryLocation and sums the bytes (1+2+3 = 6).
#[no_mangle]
pub extern "C" fn roundtrip_typed_slice_sum() -> i32 {
    let f = register_function("function(){ return new Uint8Array([1, 2, 3]); }");
    match f.invoke_for_replies(
        &[],
        ReturnTypeHints::One(ThreeState::One(ReturnTypeId::TypedArraySlice)),
    ) {
        Ok(values) => match values.into_iter().next() {
            Some(ReturnValues::TypedArraySlice(_slice_type, location)) => {
                let bytes =
                    unsafe { core::slice::from_raw_parts(location.0, location.1 as usize) };
                bytes.iter().map(|b| i32::from(*b)).sum()
            }
            _ => -1,
        },
        Err(_) => -2,
    }
}

/// NAKED object fast-path: `invoke_for_object` rides `host_invoke_function_as_object`
/// — the host interns the result in its object heap and the HANDLE crosses raw (no
/// reply encoding). Returns the handle for the JS test to resolve.
#[no_mangle]
pub extern "C" fn roundtrip_object_naked() -> i64 {
    let f = register_function("function(){ return { b: 2 }; }");
    f.invoke_for_object(&[]).clone_inner() as i64
}

/// Retires an object-heap handle (host_object_drop_external_pointer round-trip).
#[no_mangle]
pub extern "C" fn drop_object_test(handle: u64) {
    drop_object_reference(ExternalPointer::pointer(handle));
}

/// Interns a string, drops it, returns the handle — the JS test asserts eviction.
#[no_mangle]
pub extern "C" fn cache_and_drop_string() -> i64 {
    let handle = cache_text("drop-me").clone_inner();
    drop_cached_string(handle);
    handle as i64
}

// ── V2 batch codec round-trips (Operations + quantized params + group returns) ──

/// BATCH returning path: one batch carries MakeFunction (register `x*3` at a
/// pre-allocated handle) + Invoke with a quantized Int32 param and a One(Int32) hint.
/// `batch_response` decodes the group-return frame: results[0] is MakeFunction's
/// ExternalReference, results[1] the invoke result. Returns that Int32.
#[no_mangle]
pub extern "C" fn batch_register_invoke_i32(input: i32) -> i32 {
    let handle = allocate_function_reference();
    let instructions = internal_api::create_instructions(128, 128);
    if instructions
        .register_function(handle, "function(x){ return x * 3; }")
        .is_err()
    {
        return -3;
    }
    if instructions
        .invoke(
            handle,
            Some(&[Params::Int32(input)]),
            ReturnTypeHints::One(ThreeState::One(ReturnTypeId::Int32)),
        )
        .is_err()
    {
        return -4;
    }
    let Ok(completed) = instructions.complete() else {
        return -5;
    };
    match batch_response(completed) {
        Ok(results) => {
            let mut it = results.into_iter();
            match it.next() {
                Some(Returns::One(ReturnValues::ExternalReference(_))) => {}
                _ => return -6,
            }
            match it.next() {
                Some(Returns::One(ReturnValues::Int32(v))) => v,
                _ => -1,
            }
        }
        Err(_) => -2,
    }
}

/// BATCH no-return path: MakeFunction + Invoke(None hint) with mixed quantized params
/// (Int32, Text8 via the TEXTS buffer, Bool, Float64→f32-quantized). The JS fn captures
/// the decoded args onto `this` for the test to assert.
#[no_mangle]
pub extern "C" fn batch_capture_mixed_params() {
    let handle = allocate_function_reference();
    let instructions = internal_api::create_instructions(128, 256);
    instructions
        .register_function(handle, "function(){ this.batch_captured = Array.from(arguments); }")
        .expect("register in batch");
    instructions
        .invoke(
            handle,
            Some(&[
                Params::Int32(10),
                Params::Text8("hi"),
                Params::Bool(true),
                Params::Float64(2.5),
            ]),
            ReturnTypeHints::None,
        )
        .expect("invoke in batch");
    batch(instructions.complete().expect("complete batch"));
}

/// BATCH async path: MakeFunction(async `x+9`) + InvokeAsync with a caller-supplied
/// callback id and One(Int32) hint. JS resolves the Promise and delivers the framed
/// reply via invoke_callback — the test spies the callback registry.
#[no_mangle]
pub extern "C" fn batch_invoke_async(callback_id: u64) {
    let handle = allocate_function_reference();
    let instructions = internal_api::create_instructions(128, 128);
    instructions
        .register_function(handle, "async function(x){ return x + 9; }")
        .expect("register in batch");
    instructions
        .invoke_async(
            handle,
            InternalPointer::pointer(callback_id),
            Some(&[Params::Int32(1)]),
            ReturnTypeHints::One(ThreeState::One(ReturnTypeId::Int32)),
        )
        .expect("invoke_async in batch");
    batch(instructions.complete().expect("complete batch"));
}

/// BATCH quantization spread: invokes `a+b` where `a` fits an i8 (quantized) and `b`
/// needs the full i32 (TypeOptimization::None) — exercises both decoder branches.
#[no_mangle]
pub extern "C" fn batch_quantized_spread() -> i32 {
    let handle = allocate_function_reference();
    let instructions = internal_api::create_instructions(128, 128);
    if instructions
        .register_function(handle, "function(a, b){ return a + b; }")
        .is_err()
    {
        return -3;
    }
    if instructions
        .invoke(
            handle,
            Some(&[Params::Int32(7), Params::Int32(100_000)]),
            ReturnTypeHints::One(ThreeState::One(ReturnTypeId::Int32)),
        )
        .is_err()
    {
        return -4;
    }
    let Ok(completed) = instructions.complete() else {
        return -5;
    };
    match batch_response(completed) {
        Ok(results) => match results.into_iter().nth(1) {
            Some(Returns::One(ReturnValues::Int32(v))) => v,
            _ => -1,
        },
        Err(_) => -2,
    }
}

/// Registers a fn that records all decoded args onto `this` (JS-side capture), invoked
/// with a mix of param types incl. a Text8 (pointer into WASM memory). None return.
#[no_mangle]
pub extern "C" fn capture_mixed_params() {
    let f = register_function("function(){ this.captured = Array.from(arguments); }");
    f.invoke_no_return(&[
        Params::Int32(10),
        Params::Text8("hi"),
        Params::Bool(true),
        Params::Float64(2.5),
    ]);
}

// ── F27: Trigger dispatch through host_apply ───────────────────────────────

/// Frames a JSON payload as a capability trigger (protocol byte 3) and ships
/// it through `host_apply`. The JS ProtocolDispatcher routes to the registered
/// capability trigger handler (set up via `FoundationWasm._capTriggerHandler`).
///
/// # Safety
///
/// Caller must ensure `json_ptr`/`json_len` point to valid UTF-8 JSON.
#[no_mangle]
pub extern "C" fn e2e_trigger_capability(json_ptr: u64, json_len: u32) {
    let payload = unsafe { core::slice::from_raw_parts(json_ptr as *const u8, json_len as usize) };

    let total = (foundation_wasm::WasmEnvelope::HEADER_LEN + payload.len()) as u64;
    let mem_id = foundation_wasm::exposed_runtime::create_allocation(total);

    let framed = foundation_wasm::WasmEnvelope::write(3, 0, mem_id, payload);
    let slot =
        foundation_wasm::internal_api::get_memory(foundation_wasm::MemoryId::from_u64(mem_id));
    slot.apply(|m| {
        m.clear();
        m.extend_from_slice(&framed);
    });

    let (ptr, len) = slot.as_address().expect("slot address");
    unsafe { foundation_wasm::abi::web::host_apply(mem_id, ptr as u64, len) };
}

/// Same as e2e_trigger_capability but uses protocol byte 4 (IPC trigger).
#[no_mangle]
pub extern "C" fn e2e_trigger_ipc(json_ptr: u64, json_len: u32) {
    let payload = unsafe { core::slice::from_raw_parts(json_ptr as *const u8, json_len as usize) };

    let total = (foundation_wasm::WasmEnvelope::HEADER_LEN + payload.len()) as u64;
    let mem_id = foundation_wasm::exposed_runtime::create_allocation(total);

    let framed = foundation_wasm::WasmEnvelope::write(4, 0, mem_id, payload);
    let slot =
        foundation_wasm::internal_api::get_memory(foundation_wasm::MemoryId::from_u64(mem_id));
    slot.apply(|m| {
        m.clear();
        m.extend_from_slice(&framed);
    });

    let (ptr, len) = slot.as_address().expect("slot address");
    unsafe { foundation_wasm::abi::web::host_apply(mem_id, ptr as u64, len) };
}
