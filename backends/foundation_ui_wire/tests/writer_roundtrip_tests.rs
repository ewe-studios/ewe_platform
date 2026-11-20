use foundation_ui_wire::{
    read_frame,
    writer::{write_messages, ClientCaps, FrameHeader, WriterConfig},
    MessageT, OperationT, SetTextT, UiOperationT, SCHEMA_VERSION,
};

const HEADER: FrameHeader = FrameHeader {
    stream_id: 7,
    sequence_id: 20,
    ack: 5,
};

#[test]
fn write_then_read_gives_a_message_frame() {
    let messages = vec![table!(MessageT {
        operation: Some(Box::new(table!(OperationT {
            ui: UiOperationT::SetText(Box::new(table!(SetTextT {
                node_id: 42,
                text: Some("1".into()),
            }))),
        }))),
    })];
    let config = WriterConfig::default();
    let (bytes, report) = write_messages(
        messages.clone(),
        &ClientCaps::web_current(),
        HEADER,
        &config,
    )
    .unwrap();
    assert_eq!(report.filtered, []);
    assert!(report.replaced_kinds.is_empty());
    let frame = read_frame(&bytes, config.wire_limits()).unwrap();
    assert_eq!(frame.schema_version(), SCHEMA_VERSION);
    assert_eq!(frame.stream_id(), HEADER.stream_id);
    assert_eq!(frame.sequence_id(), HEADER.sequence_id);
    let owned = frame.kind_as_frame_message().unwrap().unpack();
    assert_eq!(owned.ack, HEADER.ack);
    assert_eq!(owned.messages, Some(messages));
}

#[test]
fn single_message_wrappers_keep_header_and_payload() {
    use foundation_ui_wire::{
        writer::{write_arrow_ipc, write_caps, write_error, write_messagepack},
        ActionErrorT, ErrorCode, ServerCapsT,
    };

    let config = WriterConfig::default();
    let error = table!(ActionErrorT {
        code: ErrorCode::ActionFailed,
        message: Some("failed".into())
    });
    let caps = table!(ServerCapsT {
        vocabulary_version: 2,
        stream_id: 19
    });
    let data = [0x81, 0x01, 0x02];
    let frames = [
        write_error(&error, HEADER, &config).unwrap(),
        write_caps(&caps, HEADER, &config).unwrap(),
        write_arrow_ipc(9, &data, HEADER, &config).unwrap(),
        write_messagepack(9, &data, HEADER, &config).unwrap(),
    ];
    let messages = frames.each_ref().map(|bytes| {
        let frame = read_frame(bytes, config.wire_limits()).unwrap();
        assert_eq!(frame.schema_version(), SCHEMA_VERSION);
        assert_eq!(frame.stream_id(), HEADER.stream_id);
        assert_eq!(frame.sequence_id(), HEADER.sequence_id);
        let message = frame.kind_as_frame_message().unwrap();
        assert_eq!(message.ack(), HEADER.ack);
        let message_list = message.messages().unwrap();
        assert_eq!(message_list.len(), 1);
        message_list.get(0)
    });
    assert_eq!(
        messages[0].control().unwrap().error().unwrap().unpack(),
        error
    );
    assert_eq!(
        messages[1].control().unwrap().caps().unwrap().unpack(),
        caps
    );

    let arrow_data = messages[2].data().unwrap();
    assert_eq!(arrow_data.target_node(), 9);
    let arrow = arrow_data.arrow().unwrap();
    assert_eq!(arrow.target_node(), 9);
    let ipc = arrow.bytes().unwrap().bytes();
    assert_eq!(ipc, data);
    assert_eq!((ipc.as_ptr() as usize - frames[2].as_ptr() as usize) % 8, 0);

    let packed_data = messages[3].data().unwrap();
    assert_eq!(packed_data.target_node(), 9);
    let packed = packed_data.msgpack().unwrap();
    assert_eq!(packed.target_node(), 9);
    assert_eq!(packed.bytes().unwrap().bytes(), data);
}

#[test]
fn all_wrappers_enforce_the_frame_budget() {
    use foundation_ui_wire::{
        writer::{write_arrow_ipc, write_caps, write_error, write_messagepack, WriteError},
        ActionErrorT, ServerCapsT, WireLimits,
    };
    let config = WriterConfig::default().limits(WireLimits::default().max_frame_bytes(8));
    let results = [
        write_error(&ActionErrorT::default(), HEADER, &config),
        write_caps(&ServerCapsT::default(), HEADER, &config),
        write_arrow_ipc(9, b"Arrow", HEADER, &config),
        write_messagepack(9, b"MessagePack", HEADER, &config),
    ];
    for result in results {
        assert!(matches!(result, Err(WriteError::TooLarge { limit: 8, actual }) if actual > 8));
    }
}
