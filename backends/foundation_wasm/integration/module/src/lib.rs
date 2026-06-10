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
use foundation_wasm::abi::web::host_apply;
use foundation_wasm::{exposed_runtime, internal_api, MemoryId, WasmEnvelope};

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
