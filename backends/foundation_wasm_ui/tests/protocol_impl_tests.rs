//! WHY: Layer 3 composes the Layer-1 encoders with the Layer-2 transport. These
//! tests prove each protocol impl frames exactly one arena slot with the right
//! `WasmEnvelope`, that the payload round-trips, that `ack` frees the slot, and that
//! `InstructionReceiver` batches/flushes per decision 030.
//!
//! WHAT: `ColumnarV1`/`BatchInstructionsV1`/`JsonV1` `encode_and_send` + `handle_received` +
//! `ack`, and `InstructionReceiver` queue/flush/ack behaviour.
//!
//! HOW: Native build — `host_apply` is a no-op stub, so we inspect the arena slot the
//! impl wrote and decode it back through `handle_received`.

use foundation_ui_traits::DomOp;
use foundation_wasm::{MemoryAllocations, WasmEnvelope};
use foundation_wasm_ui::{ColumnarV1, BatchInstructionsV1, InstructionReceiver, JsonV1, ProtocolMethods};

fn sample_ops() -> Vec<DomOp> {
    vec![
        DomOp::CreateElement {
            node_id: 1000,
            tag: "div".into(), // known tag -> rides the wire as `id:1`
            class: "card".into(),
        },
        DomOp::RegisterNode { node_id: 1000 },
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
fn columnar_v1_encode_send_round_trip() {
    assert_protocol_round_trip(&ColumnarV1::new(), 1);
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

// ─── Feature 01 spec tests 22-24: byte-0 (Custom Binary) round-trips ───────────
// The CustomBinaryEncoder of the spec IS BatchInstructionsV1 (decision 022), so
// the spec's encoder tests live here rather than in foundation_ui_traits.

/// Test 23 — an empty batch ships and decodes to an empty vec.
#[test]
fn batch_instructions_empty_batch_round_trips() {
    let mut mem = MemoryAllocations::new();
    let proto = BatchInstructionsV1::new();
    let result = proto.encode_and_send(vec![], &mut mem);
    let slot = mem.get(result.memory_id).expect("slot live");
    let bytes = slot.clone_memory().expect("read bytes");
    let (_, payload) = WasmEnvelope::parse(&bytes);
    let decoded = proto
        .handle_received(result.memory_id, payload.as_ptr(), payload.len())
        .expect("decode");
    assert_eq!(decoded, vec![]);
}

/// Test 24 — every `DomOp` variant survives the byte-0 instruction stream.
#[test]
fn batch_instructions_round_trips_all_nineteen_variants() {
    use foundation_ui_traits::{MorphAction, TargetSelector};
    use std::borrow::Cow;

    let ops = vec![
        DomOp::CreateElement {
            node_id: 1,
            tag: "custom-widget".into(),
            class: "".into(),
        },
        DomOp::CreateTextNode {
            node_id: 2,
            content: "text".into(),
        },
        DomOp::SetText {
            node_id: 3,
            text: "new".into(),
        },
        DomOp::SetAttribute {
            node_id: 4,
            name: "class".into(),
            value: "x".into(),
        },
        DomOp::RemoveAttribute {
            node_id: 5,
            name: "style".into(),
        },
        DomOp::SetProperty {
            node_id: 6,
            name: "value".into(),
            value: "\"v\"".into(),
        },
        DomOp::AddEventListener {
            node_id: 7,
            event_name: "click".into(),
        },
        DomOp::RemoveEventListener {
            node_id: 8,
            event_name: "click".into(),
        },
        DomOp::AppendChild {
            parent_id: 9,
            child_id: 10,
        },
        DomOp::RemoveChild {
            parent_id: 11,
            child_id: 12,
        },
        DomOp::RemoveNode { node_id: 13 },
        DomOp::InsertBefore {
            parent_id: 14,
            child_id: 15,
            ref_id: 16,
        },
        DomOp::ReplaceNode {
            old_id: 17,
            new_id: 18,
        },
        DomOp::SetStyle {
            node_id: 19,
            prop: "color".into(),
            value: "red".into(),
        },
        DomOp::AddClass {
            node_id: 20,
            class: "on".into(),
        },
        DomOp::RemoveClass {
            node_id: 21,
            class: "off".into(),
        },
        DomOp::MorphNode {
            target: TargetSelector::Query(Cow::Borrowed("main > p:last-child")),
            action: MorphAction::InsertAfter,
            content: "<b>m</b>".into(),
        },
        DomOp::RegisterNode { node_id: 22 },
        DomOp::UnregisterNode { node_id: 23 },
    ];
    assert_eq!(ops.len(), 19);

    let mut mem = MemoryAllocations::new();
    let proto = BatchInstructionsV1::new();
    let result = proto.encode_and_send(ops.clone(), &mut mem);
    let slot = mem.get(result.memory_id).expect("slot live");
    let bytes = slot.clone_memory().expect("read bytes");
    let (_, payload) = WasmEnvelope::parse(&bytes);
    let decoded = proto
        .handle_received(result.memory_id, payload.as_ptr(), payload.len())
        .expect("decode");
    assert_eq!(decoded, ops);
}

#[test]
fn instruction_receiver_batches_and_flushes_once() {
    let ops = sample_ops();
    let mut receiver = InstructionReceiver::new(Box::new(ColumnarV1::new()), MemoryAllocations::new());

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
    assert_eq!(envelope.protocol, 1); // columnar (Arrow slot, wire v1)
    let decoded = ColumnarV1::new()
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
    let mut receiver = InstructionReceiver::with_global_arena(Box::new(ColumnarV1::new()));
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

    let decoded = ColumnarV1::new()
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
