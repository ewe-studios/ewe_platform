//! Verify typed operation delivery through the public frame protocol.
use foundation_ui_wire::{ops, read_frame, WireLimits};
use foundation_wasm::{MemoryAllocations, ProtocolHandler};
use foundation_wasm_ui::{FrameV1, InstructionReceiver, ProtocolError, ProtocolMethods};

#[test]
fn delivery_errors_keep_sequence_and_header() {
    let codec = FrameV1::new(41);
    let result = codec.deliver(vec![ops::set_text(2, "first")], 3, |_| {
        Err::<(), _>(ProtocolError::Delivery("closed".into()))
    });
    assert!(matches!(result, Err(ProtocolError::Delivery(_))));
    let bytes = codec
        .deliver(vec![ops::set_text(2, "second")], 7, Ok)
        .unwrap();
    let frame = read_frame(&bytes, &WireLimits::default()).unwrap();
    assert_eq!(
        (
            frame.stream_id(),
            frame.sequence_id(),
            frame.kind_as_frame_message().unwrap().ack()
        ),
        (41, 1, 7)
    );
    assert_eq!(
        codec.handle_received(&bytes).unwrap(),
        vec![ops::set_text(2, "second")]
    );
}

#[test]
fn handoff_owns_one_complete_slot_until_acknowledged() {
    let codec = FrameV1::new(9);
    let mut arena = MemoryAllocations::new();
    let ops = vec![ops::set_text(4, "one"), ops::set_text(7, "two")];
    let (sent, ptr, len) = codec.encode_and_write(ops.clone(), &mut arena).unwrap();
    let slot = arena.get(sent.memory_id).unwrap();
    slot.into_with(|frame| {
        assert_eq!(&frame[4..8], b"EWF1");
        assert_eq!((frame.as_ptr(), frame.len()), (ptr, len));
        assert_eq!(codec.handle_received(frame).unwrap(), ops);
    })
    .unwrap();
    codec.ack(sent.memory_id, &mut arena);
    assert!(arena.get(sent.memory_id).is_err());
}

#[test]
fn instruction_receiver_sends_a_bare_frame() {
    let mut out = InstructionReceiver::new(Box::new(FrameV1::new(3)), MemoryAllocations::new());
    out.queue(ops::set_text(6, "ready"));
    let sent = out.flush().unwrap().unwrap();
    let slot = out.memory().unwrap().get(sent.memory_id).unwrap();
    slot.into_with(|frame| assert_eq!(&frame[4..8], b"EWF1"))
        .unwrap();
    out.ack(sent.memory_id);
}

#[test]
fn rejected_batch_skips_delivery_and_leaves_next_sequence_available() {
    use foundation_ui_wire::writer::{ClientCaps, WriterConfig};
    let config = WriterConfig::default().limits(WireLimits::default().max_frame_bytes(200));
    let codec = FrameV1::with_config(41, ClientCaps::web_current(), config);
    let too_big = vec![ops::set_text(2, &"x".repeat(512))];
    let result = codec.deliver(too_big, 0, |_| -> Result<(), ProtocolError> {
        panic!("invalid batch sent")
    });
    assert!(matches!(result, Err(ProtocolError::Writer(_))));
    let bytes = codec.deliver(vec![ops::set_text(2, "ok")], 0, Ok).unwrap();
    assert_eq!(
        read_frame(&bytes, &WireLimits::default())
            .unwrap()
            .sequence_id(),
        1
    );
}
