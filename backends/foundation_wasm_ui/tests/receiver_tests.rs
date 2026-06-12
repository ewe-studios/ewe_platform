//! WHY: The `InstructionReceiver` is the single funnel between the reactive
//! graph and the wire — its batching, ordering, no-dedup, capacity, and
//! memory-lifecycle guarantees (feature 04 section 12) are what every effect
//! silently relies on.
//!
//! WHAT: Spec tests 1-29 — queue/flush against [`MockProtocol`], protocol
//! integration (Arrow/byte-0/JSON `SendResult` fields + ack), arena lifecycle
//! (generation checks, leak-freedom), `RuntimeBuilder` contract, and the
//! stabilize→flush signals loop end-to-end.

use std::rc::Rc;

use foundation_signals::{Context, Runtime as SignalsRuntime};
use foundation_ui_traits::{DomOp, ProtocolEncoder};
use foundation_wasm::{MemoryAllocations, ProtocolHandler, WasmEnvelope};
use foundation_wasm_ui::{
    ArrowV1, BatchInstructionsV1, DomSignalBinding, InstructionReceiver, JsonV1, MockProtocol,
    ProtocolMethods, Runtime,
};

fn set_text(node_id: u32, text: &str) -> DomOp {
    DomOp::SetText {
        node_id,
        text: String::from(text).into(),
    }
}

/// A mock-backed receiver plus the shared recorders.
fn mock_receiver() -> (
    InstructionReceiver,
    Rc<std::cell::RefCell<Vec<Vec<DomOp>>>>,
) {
    let mock = MockProtocol::new();
    let sent = mock.sent_batches();
    let receiver = InstructionReceiver::new(Box::new(mock), MemoryAllocations::new());
    (receiver, sent)
}

// ─── Queue & flush (tests 1-9) ─────────────────────────────────────────────────

/// Tests 1 + 7 — one batch, exact insertion order, no reordering by node.
#[test]
fn flush_ships_one_batch_in_queue_order() {
    let (mut receiver, sent) = mock_receiver();
    receiver.queue(set_text(1, "a"));
    receiver.queue(DomOp::AddClass {
        node_id: 1,
        class: "x".into(),
    });
    receiver.queue(set_text(2, "b"));

    let result = receiver.flush().expect("non-empty flush ships");
    assert_eq!(result.op_count, 3);
    let batches = sent.borrow();
    assert_eq!(batches.len(), 1);
    assert_eq!(
        batches[0],
        vec![
            set_text(1, "a"),
            DomOp::AddClass {
                node_id: 1,
                class: "x".into()
            },
            set_text(2, "b"),
        ]
    );
}

/// Tests 2 + 4 + 23 + 24 — empty/double flushes are no-ops.
#[test]
fn empty_and_double_flushes_are_noops() {
    let (mut receiver, sent) = mock_receiver();
    assert!(receiver.flush().is_none());
    assert_eq!(receiver.flush_count(), 0);

    receiver.queue(set_text(1, "x"));
    receiver.flush().unwrap();
    assert!(receiver.flush().is_none(), "second flush has nothing");
    assert_eq!(sent.borrow().len(), 1);
    assert_eq!(receiver.flush_count(), 1, "empty flush did not increment");
}

/// Tests 3 + 6 — separate cycles produce separate batches; `flush_count` counts
/// only non-empty flushes.
#[test]
fn flush_cycles_batch_separately() {
    let (mut receiver, sent) = mock_receiver();
    for i in 0..5 {
        receiver.queue(set_text(i, "first"));
    }
    receiver.flush().unwrap();
    for i in 0..3 {
        receiver.queue(set_text(i, "second"));
    }
    receiver.flush().unwrap();
    receiver.queue(set_text(9, "third"));
    receiver.flush().unwrap();

    let batches = sent.borrow();
    assert_eq!(batches.len(), 3);
    assert_eq!(batches[0].len(), 5);
    assert_eq!(batches[1].len(), 3);
    assert_eq!(batches[2].len(), 1);
    assert_eq!(receiver.flush_count(), 3);
}

/// Tests 5 + 26 — large batches ship complete and ordered.
#[test]
fn large_batches_keep_count_and_order() {
    let (mut receiver, sent) = mock_receiver();
    for i in 0..5000u32 {
        receiver.queue(set_text(i, "v"));
    }
    assert_eq!(receiver.pending_count(), 5000);
    let result = receiver.flush().unwrap();
    assert_eq!(result.op_count, 5000);
    let batches = sent.borrow();
    assert_eq!(batches[0].len(), 5000);
    for (i, op) in batches[0].iter().enumerate() {
        let DomOp::SetText { node_id, .. } = op else {
            panic!("unexpected op at {i}");
        };
        assert_eq!(*node_id, u32::try_from(i).unwrap());
    }
}

/// Test 9 — same node, different ops: NO dedup (order-dependent stream).
#[test]
fn no_deduplication_for_same_node() {
    let (mut receiver, sent) = mock_receiver();
    receiver.queue(set_text(7, "one"));
    receiver.queue(set_text(7, "two"));
    receiver.flush().unwrap();
    assert_eq!(sent.borrow()[0].len(), 2, "both ops present");
}

// ─── Protocol integration (tests 10-14) ────────────────────────────────────────

fn five_ops() -> Vec<DomOp> {
    vec![
        DomOp::CreateElement {
            node_id: 10,
            tag: "div".into(),
            class: "c".into(),
        },
        DomOp::RegisterNode { node_id: 10 },
        set_text(10, "hello"),
        DomOp::AppendChild {
            parent_id: 1,
            child_id: 10,
        },
        DomOp::RemoveNode { node_id: 99 },
    ]
}

/// Test 10 — `ArrowV1` `SendResult` diagnostics.
#[test]
fn arrow_send_result_carries_counts() {
    let mut memory = MemoryAllocations::new();
    let result = ArrowV1::new().encode_and_send(five_ops(), &mut memory);
    assert_eq!(result.op_count, 5);
    assert!(result.encoded_bytes > 0);
    assert!(memory.get(result.memory_id).is_ok(), "slot is live");
}

/// Test 11 — byte-0 payload layout: one slot, `texts_off`/`texts_len` header.
#[test]
fn batch_instructions_payload_is_one_slot_with_text_pool() {
    let mut memory = MemoryAllocations::new();
    let proto = BatchInstructionsV1::new();
    let result = proto.encode_and_send(five_ops(), &mut memory);
    assert_eq!(result.op_count, 5);

    // Exactly one LIVE slot (the batch scratch slots are recycled).
    assert_eq!(memory.total_allocated() - memory.total_free(), 1);

    let bytes = memory.get(result.memory_id).unwrap().clone_memory().unwrap();
    let (envelope, payload) = WasmEnvelope::parse(&bytes);
    assert_eq!(envelope.protocol, 0);
    let texts_off = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
    let texts_len = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]) as usize;
    assert!(texts_off >= 8 && texts_off <= payload.len());
    assert_eq!(payload.len(), texts_off + texts_len, "ops + texts inline");
}

/// Test 12 — `JsonV1` slot contains JSON that `serde_json` parses.
#[test]
fn json_payload_is_valid_json() {
    let mut memory = MemoryAllocations::new();
    let result = JsonV1::new().encode_and_send(five_ops(), &mut memory);
    let bytes = memory.get(result.memory_id).unwrap().clone_memory().unwrap();
    let (_, payload) = WasmEnvelope::parse(&bytes);
    let value: serde_json::Value = serde_json::from_slice(payload).expect("valid JSON payload");
    assert_eq!(value.as_array().map(Vec::len), Some(5));
}

/// Tests 13 + 16 + 25 — ack frees; stale ids miss without panicking.
#[test]
fn ack_frees_and_stale_ids_miss() {
    let mut memory = MemoryAllocations::new();
    let proto = ArrowV1::new();
    let result = proto.encode_and_send(five_ops(), &mut memory);
    proto.ack(result.memory_id, &mut memory);
    assert!(memory.get(result.memory_id).is_err(), "freed after ack");
    // Double-ack of the now-stale id is a safe no-op.
    proto.ack(result.memory_id, &mut memory);
}

/// Test 14 — protocol byte values.
#[test]
fn protocol_bytes_match_the_spec() {
    assert_eq!(ArrowV1::new().protocol_byte(), 1);
    assert_eq!(BatchInstructionsV1::new().protocol_byte(), 0);
    assert_eq!(JsonV1::new().protocol_byte(), 2);
    assert_eq!(MockProtocol::new().protocol_byte(), 255);
}

// ─── Memory lifecycle (tests 15-18) ────────────────────────────────────────────

/// Test 15 — the slot holds exactly envelope + encoded payload.
#[test]
fn slot_contains_exact_encoded_bytes() {
    let mut memory = MemoryAllocations::new();
    let ops = five_ops();
    let expected_payload = foundation_ui_traits::ArrowEncoder.encode(ops.clone());
    let result = ArrowV1::new().encode_and_send(ops, &mut memory);
    assert_eq!(result.encoded_bytes, expected_payload.len());

    let bytes = memory.get(result.memory_id).unwrap().clone_memory().unwrap();
    let (_, payload) = WasmEnvelope::parse(&bytes);
    assert_eq!(payload, expected_payload);
}

/// Test 17 — generations prevent stale access after slot recycling.
#[test]
fn generation_prevents_stale_access_after_recycle() {
    let mut memory = MemoryAllocations::new();
    let proto = ArrowV1::new();
    let first = proto.encode_and_send(five_ops(), &mut memory);
    proto.ack(first.memory_id, &mut memory);

    // Reallocate — the arena recycles the slot index with a bumped generation.
    let second = proto.encode_and_send(five_ops(), &mut memory);
    assert!(memory.get(first.memory_id).is_err(), "old id stays dead");
    assert!(memory.get(second.memory_id).is_ok());
}

/// Test 18 — N flush+ack cycles leave zero live slots.
#[test]
fn flush_ack_cycles_do_not_leak() {
    let mut receiver =
        InstructionReceiver::new(Box::new(ArrowV1::new()), MemoryAllocations::new());
    for cycle in 0..10u32 {
        receiver.queue(set_text(cycle, "tick"));
        let result = receiver.flush().unwrap();
        receiver.ack(result.memory_id);
    }
    let memory = receiver.memory().expect("owned arena");
    assert_eq!(
        memory.total_allocated() - memory.total_free(),
        0,
        "all slots returned"
    );
    assert_eq!(receiver.flush_count(), 10);
}

// ─── RuntimeBuilder (tests 19-22) ──────────────────────────────────────────────

/// Test 19 — happy-path build.
#[test]
fn builder_with_protocol_and_memory_builds() {
    let runtime = Runtime::builder()
        .protocol(ArrowV1::new())
        .memory(MemoryAllocations::new())
        .build();
    assert_eq!(runtime.receiver().pending_count(), 0);
}

/// Test 20 — missing protocol panics with the spec message.
#[test]
#[should_panic(expected = "protocol is required")]
fn builder_missing_protocol_panics() {
    let _ = Runtime::builder().memory(MemoryAllocations::new()).build();
}

/// Test 21 — missing memory panics with the spec message.
#[test]
#[should_panic(expected = "memory is required")]
fn builder_missing_memory_panics() {
    let _ = Runtime::builder().protocol(ArrowV1::new()).build();
}

/// Test 22 — ops flow through a builder-constructed runtime.
#[test]
fn builder_runtime_routes_ops_to_protocol() {
    let mock = MockProtocol::new();
    let sent = mock.sent_batches();
    let runtime = Runtime::builder()
        .protocol(mock)
        .memory(MemoryAllocations::new())
        .build();

    let receiver = runtime.receiver();
    receiver.queue(set_text(1, "via builder"));
    receiver.flush().unwrap();
    assert_eq!(sent.borrow().len(), 1);
    assert_eq!(sent.borrow()[0], vec![set_text(1, "via builder")]);
}

// ─── MockProtocol (tests 27-29) ────────────────────────────────────────────────

/// Tests 27 + 28 — the mock records batches and ACKs in order.
#[test]
fn mock_records_batches_and_acks() {
    let mock = MockProtocol::new();
    let sent = mock.sent_batches();
    let acked = mock.acked_ids();
    let mut receiver = InstructionReceiver::new(Box::new(mock), MemoryAllocations::new());

    let mut ids = Vec::new();
    for i in 0..3u32 {
        receiver.queue(set_text(i, "cycle"));
        ids.push(receiver.flush().unwrap().memory_id);
    }
    receiver.ack(ids[0]);
    receiver.ack(ids[2]);

    assert_eq!(sent.borrow().len(), 3);
    assert_eq!(*acked.borrow(), vec![ids[0], ids[2]]);
}

// ─── Stabilize→flush loop (spec section 6, test 8) ─────────────────────────────

/// Test 8 + section 6 e2e: two effects in one stabilize land in ONE batch, in
/// effect order, flushed AUTOMATICALLY by the attached notification manager.
#[test]
fn signals_stabilize_flushes_one_batch_through_the_attached_runtime() {
    let signals = Rc::new(SignalsRuntime::new());
    let ctx = Context::new(Rc::clone(&signals));

    let mock = MockProtocol::new();
    let sent = mock.sent_batches();
    let runtime = Runtime::builder()
        .protocol(mock)
        .memory(MemoryAllocations::new())
        .build();
    runtime.attach(&signals);
    let receiver = runtime.receiver();

    let (count, set_count) = ctx.signal(0i64);

    // Two bindings over the same signal — two effects, one batch.
    let _title = DomSignalBinding::bind(&ctx, &count, &receiver, 1, |v| format!("count: {v}"));
    let _badge = DomSignalBinding::bind(&ctx, &count, &receiver, 2, |v| format!("{v}"));

    // Decision 008: creation queued the initial renders; they are still pending
    // (no stabilize yet), so flush them through the loop once.
    signals.stabilize();
    assert_eq!(sent.borrow().len(), 1, "initial renders flushed as one batch");
    assert_eq!(sent.borrow()[0].len(), 2);

    set_count.set(42);
    signals.stabilize();

    let batches = sent.borrow();
    assert_eq!(batches.len(), 2, "one more batch for the change");
    assert_eq!(
        batches[1],
        vec![set_text(1, "count: 42"), set_text(2, "42")],
        "both effects' ops in one batch, effect order preserved"
    );
    assert_eq!(receiver.pending_count(), 0, "queue drained by the manager");
}

/// Changing nothing queues nothing: stabilize with no dirty effects must not
/// produce an empty protocol message.
#[test]
fn quiet_stabilize_sends_nothing() {
    let signals = Rc::new(SignalsRuntime::new());
    let ctx = Context::new(Rc::clone(&signals));
    let mock = MockProtocol::new();
    let sent = mock.sent_batches();
    let runtime = Runtime::builder()
        .protocol(mock)
        .memory(MemoryAllocations::new())
        .build();
    runtime.attach(&signals);
    let receiver = runtime.receiver();

    let (count, _set) = ctx.signal(0i64);
    let _bind = DomSignalBinding::bind(&ctx, &count, &receiver, 1, |v| format!("{v}"));
    signals.stabilize(); // flushes the initial render

    let before = sent.borrow().len();
    signals.stabilize(); // nothing dirty
    assert_eq!(sent.borrow().len(), before, "no empty batch shipped");
}
