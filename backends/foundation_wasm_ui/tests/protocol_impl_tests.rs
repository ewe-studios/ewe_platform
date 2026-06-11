//! WHY: Layer 3 composes the Layer-1 encoders with the Layer-2 transport. These
//! tests prove each protocol impl frames exactly one arena slot with the right
//! `WasmEnvelope`, that the payload round-trips, that `ack` frees the slot, and that
//! `InstructionReceiver` batches/flushes per decision 030.
//!
//! WHAT: `ArrowV1`/`BatchInstructionsV1`/`JsonV1` `encode_and_send` + `handle_received` +
//! `ack`, and `InstructionReceiver` queue/flush/ack behaviour.
//!
//! HOW: Native build — `host_apply` is a no-op stub, so we inspect the arena slot the
//! impl wrote and decode it back through `handle_received`.

use foundation_ui_traits::DomOp;
use foundation_wasm::{MemoryAllocations, WasmEnvelope};
use foundation_wasm_ui::{ArrowV1, BatchInstructionsV1, InstructionReceiver, JsonV1, ProtocolMethods};

fn sample_ops() -> Vec<DomOp> {
    vec![
        DomOp::CreateEl {
            node_id: 1000,
            tag: "div".into(),
            class: "card".into(),
        },
        DomOp::SetText {
            node_id: 1001,
            text: "hello — café".into(),
        },
        DomOp::AppendChild {
            parent_id: 1000,
            child_id: 1001,
        },
    ]
}

/// Encode a batch, assert one framed slot, decode it back, and free it.
fn assert_protocol_round_trip<P: ProtocolMethods<Vec<DomOp>>>(proto: &P, expected_byte: u8) {
    let ops = sample_ops();
    let mut mem = MemoryAllocations::new();

    let result = proto.encode_and_send(ops.clone(), &mut mem);

    // Exactly one LIVE arena slot holds the shipped message. (`total_allocated`
    // counts slots ever created — the batch protocol recycles two scratch slots
    // for its ops/texts buffers, so live = allocated - free is the invariant.)
    assert_eq!(
        mem.total_allocated() - mem.total_free(),
        1,
        "expected exactly one LIVE arena slot"
    );

    // The slot begins with a WasmEnvelope carrying this protocol byte + memory_id.
    let slot = mem.get(result.memory_id).expect("slot is live");
    let bytes = slot.clone_memory().expect("read slot bytes");
    let (envelope, payload) = WasmEnvelope::parse(&bytes);
    assert_eq!(envelope.protocol, expected_byte);
    assert_eq!(envelope.memory_id, result.memory_id.as_u64());
    assert_eq!(envelope.length as usize, payload.len());

    // The payload decodes back to the original ops.
    let decoded = proto
        .handle_received(result.memory_id, payload.as_ptr(), payload.len())
        .expect("payload decodes");
    assert_eq!(decoded, ops);

    // ack releases the slot back to the arena.
    proto.ack(result.memory_id, &mut mem);
    assert!(mem.get(result.memory_id).is_err(), "slot freed after ack");
}

#[test]
fn arrow_v1_encode_send_round_trip() {
    assert_protocol_round_trip(&ArrowV1::new(), 1);
}

#[test]
fn batch_instructions_v1_encode_send_round_trip() {
    // Byte 0 = the foundation_wasm Instructions format (decision 022): the payload
    // is [texts_off][texts_len][Operations stream][texts pool]; handle_received
    // decodes the BATCH_OP_APPLY_DOM opcodes + quantized params back to DomOps.
    assert_protocol_round_trip(&BatchInstructionsV1::new(), 0);
}

#[test]
fn json_v1_encode_send_round_trip() {
    assert_protocol_round_trip(&JsonV1::new(), 2);
}

#[test]
fn instruction_receiver_batches_and_flushes_once() {
    let ops = sample_ops();
    let mut receiver = InstructionReceiver::new(Box::new(ArrowV1::new()), MemoryAllocations::new());

    // Empty flush is a no-op: no encoding, no slot.
    assert!(receiver.flush().is_none());
    assert_eq!(receiver.memory().expect("owned arena").total_allocated(), 0);

    for op in &ops {
        receiver.queue(op.clone());
    }
    assert_eq!(receiver.pending(), ops.len());

    let result = receiver.flush().expect("flush ships a batch");
    assert_eq!(receiver.pending(), 0, "queue drained after flush");
    assert_eq!(
        receiver.memory().expect("owned arena").total_allocated(),
        1,
        "one slot for the whole batch"
    );

    // The single slot decodes back to all queued ops.
    let slot = receiver.memory().expect("owned arena").get(result.memory_id).expect("slot live");
    let bytes = slot.clone_memory().expect("read bytes");
    let (envelope, payload) = WasmEnvelope::parse(&bytes);
    assert_eq!(envelope.protocol, 1); // Arrow
    let decoded = ArrowV1::new()
        .handle_received(result.memory_id, payload.as_ptr(), payload.len())
        .expect("decode");
    assert_eq!(decoded, ops);

    receiver.ack(result.memory_id);
    assert!(receiver.memory().expect("owned arena").get(result.memory_id).is_err());
}

#[test]
fn instruction_receiver_empty_flush_does_nothing() {
    let mut receiver = InstructionReceiver::new(Box::new(JsonV1::new()), MemoryAllocations::new());
    assert!(receiver.flush().is_none());
    assert_eq!(receiver.pending(), 0);
    assert_eq!(receiver.memory().expect("owned arena").total_allocated(), 0);
}

#[test]
fn instruction_receiver_global_arena_slots_are_visible_to_the_exposed_runtime() {
    // The live-loop constructor: slots must land in the GLOBAL arena — the one JS's
    // `dispose_allocation` export frees (the feature-00 arena seam). We verify the
    // shipped slot resolves through `internal_api::get_memory` (same arena the
    // exposed_runtime exports use) and decodes back to the queued ops.
    let ops = sample_ops();
    let mut receiver = InstructionReceiver::with_global_arena(Box::new(ArrowV1::new()));
    assert!(receiver.memory().is_none(), "global receiver owns no arena");

    for op in &ops {
        receiver.queue(op.clone());
    }
    let result = receiver.flush().expect("flush ships a batch");

    let slot = foundation_wasm::internal_api::get_memory(result.memory_id);
    let bytes = slot.clone_memory().expect("read slot bytes from the GLOBAL arena");
    let (envelope, payload) = WasmEnvelope::parse(&bytes);
    assert_eq!(envelope.protocol, 1);
    assert_eq!(envelope.memory_id, result.memory_id.as_u64());

    let decoded = ArrowV1::new()
        .handle_received(result.memory_id, payload.as_ptr(), payload.len())
        .expect("payload decodes");
    assert_eq!(decoded, ops);

    // ACK through the receiver (host-side path) — the global slot is freed.
    receiver.ack(result.memory_id);
    foundation_wasm::internal_api::with_global_allocations(|memory| {
        assert!(memory.get(result.memory_id).is_err(), "global slot freed after ack");
    });
}

#[cfg(feature = "embedded-js")]
#[test]
fn embedded_js_assets_carry_both_runtimes() {
    use foundation_wasm_ui::embedded::{FOUNDATION_WASM_JS, FOUNDATION_WASM_UI_JS};
    assert!(FOUNDATION_WASM_JS.contains("class FoundationWasm"));
    assert!(FOUNDATION_WASM_JS.contains("globalThis.FoundationWasmRuntime"));
    assert!(FOUNDATION_WASM_UI_JS.contains("globalThis.FoundationWasmUiRuntime"));
    assert!(FOUNDATION_WASM_UI_JS.contains("class DomHeap"));
}
