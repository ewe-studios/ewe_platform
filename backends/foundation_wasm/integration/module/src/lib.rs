//! Minimal wasm32 module that exercises the real foundation_wasm ABI so
//! `foundation-wasm.js` can be tested end-to-end against an actual instance.
//!
//! `emit_arrow_batch` allocates a slot in the global arena, frames an Arrow-encoded
//! `DomOp` batch in a `WasmEnvelope`, and ships it through the uniform `host_apply`
//! import — the exact WASM→JS transport path the JS dispatcher consumes.
//!
//! This is a `std` crate (like the other nodejs integration fixtures) so it links the
//! default allocator + panic handler; `foundation_wasm` itself stays `no_std`.

use foundation_ui_traits::{ArrowEncoder, DomOp, ProtocolEncoder};
use foundation_wasm::abi::web::{
    host_apply, invoke_as_bool, invoke_as_f64, invoke_as_i32, register_function,
};
use foundation_wasm::{
    exposed_runtime, internal_api, MemoryId, Params, ReturnTypeHints, ReturnTypeId, ReturnValues,
    ThreeState, WasmEnvelope,
};

/// Build a 2-op Arrow batch and ship it to JS via `host_apply`.
///
/// # Panics
/// Panics if the arena slot can't be addressed (never in practice).
#[no_mangle]
pub extern "C" fn emit_arrow_batch() {
    let ops = vec![
        DomOp::SetText {
            node_id: 5,
            text: "hi".to_string(),
        },
        DomOp::Remove { node_id: 6 },
    ];
    let payload = ArrowEncoder.encode(ops);

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
