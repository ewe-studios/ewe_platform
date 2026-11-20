use std::fs;
use std::path::Path;

use flatbuffers::FlatBufferBuilder;
use foundation_ui_wire::{
    expect_kind, finish_frame, frame_kind, read_frame, read_signed_state, FrameKindName, FrameT,
    Operation, UiOperation, UiOperationT, WireError, WireLimits, FRAME_IDENTIFIER,
    STATE_IDENTIFIER,
};

#[test]
fn frame_above_limit_is_rejected_before_verify() {
    let bytes = [0xff; 2048];
    let limits = WireLimits::default().max_frame_bytes(1024);
    assert_eq!(
        read_frame(&bytes, &limits),
        Err(WireError::TooLarge {
            limit: 1024,
            actual: bytes.len(),
        })
    );
    assert_eq!(
        read_frame(&bytes[..5], &limits.max_frame_bytes(4)),
        Err(WireError::TooLarge {
            limit: 4,
            actual: 5
        })
    );
}

#[test]
fn random_bytes_are_rejected() {
    let mut bytes = [0xff; 16];
    bytes[4..8].copy_from_slice(FRAME_IDENTIFIER.as_bytes());
    assert!(matches!(
        read_frame(&bytes, &WireLimits::default()),
        Err(WireError::Invalid { .. })
    ));
}

#[test]
fn truncated_header_is_rejected() {
    let bytes = [0; 7];
    assert_eq!(
        read_frame(&bytes, &WireLimits::default()),
        Err(WireError::TooShort { actual: 7 })
    );
}

#[test]
fn wrong_identifier_is_rejected() {
    let limits = WireLimits::default();
    let frame = fixture("frame_ops_counter.bin");
    let state = fixture("state_counter.bin");
    assert_eq!(
        read_frame(&state, &limits),
        Err(WireError::WrongIdentifier {
            expected: FRAME_IDENTIFIER,
            found: *b"EWS1",
        })
    );
    assert_eq!(
        read_signed_state(&frame, &limits),
        Err(WireError::WrongIdentifier {
            expected: STATE_IDENTIFIER,
            found: *b"EWF1",
        })
    );
}

#[test]
fn wrong_kind_is_rejected() {
    let bytes = fixture("action_increment.bin");
    let request = read_frame(&bytes, &WireLimits::default()).unwrap().unpack();
    assert_eq!(frame_kind(&request), Some(FrameKindName::Request));
    assert_eq!(expect_kind(&request, FrameKindName::Request), Ok(()));
    assert_eq!(
        expect_kind(&request, FrameKindName::Message),
        Err(WireError::WrongKind {
            expected: FrameKindName::Message,
            found: FrameKindName::Request,
        })
    );
}

#[test]
fn no_kind_is_rejected() {
    let frame = FrameT::default();
    assert_eq!(frame_kind(&frame), None);
    assert_eq!(
        expect_kind(&frame, FrameKindName::Message),
        Err(WireError::NoKind)
    );
    assert_eq!(finish_frame(&frame), Err(WireError::NoKind));

    let mut builder = FlatBufferBuilder::new();
    let offset = frame.pack(&mut builder);
    builder.finish(offset, Some(FRAME_IDENTIFIER));
    assert_eq!(
        read_frame(builder.finished_data(), &WireLimits::default()),
        Err(WireError::NoKind)
    );
}

#[test]
fn action_limit_applies_to_action_frames_and_state() {
    let limits = WireLimits::default().max_action_request_bytes(1);
    let action = fixture("action_increment.bin");
    assert_eq!(
        read_frame(&action, &limits),
        Err(WireError::TooLarge {
            limit: 1,
            actual: action.len()
        })
    );
    let state = fixture("state_counter.bin");
    assert_eq!(
        read_signed_state(&state, &limits),
        Err(WireError::TooLarge {
            limit: 1,
            actual: state.len()
        })
    );
    let hello = fixture("hello_web.bin");
    assert!(read_frame(&hello, &limits).is_ok());
}

#[test]
fn verifier_uses_depth_and_table_limits() {
    let bytes = fixture("frame_ops_counter.bin");
    for limits in [
        WireLimits::default().max_depth(1),
        WireLimits::default().max_tables(1),
    ] {
        assert!(matches!(
            read_frame(&bytes, &limits),
            Err(WireError::Invalid { .. })
        ));
    }
}

#[test]
fn unknown_union_member_reads_as_none() {
    let bytes = fixture("frame_unknown_op.bin");
    let frame = read_frame(&bytes, &WireLimits::default()).unwrap();
    let message = frame.kind_as_frame_message().unwrap();
    let operation = message.messages().unwrap().get(0).operation().unwrap();
    assert_eq!(operation.ui_type(), UiOperation(250));
    assert!(operation.ui_as_set_text().is_none());
    assert!(matches!(operation.unpack().ui, UiOperationT::NONE));
    assert_eq!(message.ack(), 5);
    assert_eq!(frame.stream_id(), 7);
    assert_eq!(frame.sequence_id(), 20);
}

#[test]
fn unknown_operation_does_not_hide_known_message_fields() {
    use foundation_ui_wire::{ActionErrorT, ControlT, ErrorCode, FrameKindT};

    let limits = WireLimits::default();
    let bytes = fixture("frame_ops_counter.bin");
    let mut frame = read_frame(&bytes, &limits).unwrap().unpack();
    let FrameKindT::FrameMessage(body) = &mut frame.kind else {
        panic!("Expected a message frame");
    };
    body.messages.as_mut().unwrap()[0].control = Some(Box::new(table!(ControlT {
        error: Some(Box::new(table!(ActionErrorT {
            code: ErrorCode::ActionFailed,
            message: Some("failed".into()),
        }))),
    })));
    let mut bytes = finish_frame(&frame).unwrap();
    let operation = first_message(&bytes, &limits).operation().unwrap();
    let position =
        operation._tab.loc() + usize::from(operation._tab.vtable().get(Operation::VT_UI_TYPE));
    bytes[position] = 250;

    let message = first_message(&bytes, &limits);
    let operation = message.operation().unwrap();
    assert_eq!(operation.ui_type(), UiOperation(250));
    assert!(matches!(operation.unpack().ui, UiOperationT::NONE));
    assert_eq!(
        message.control().unwrap().error().unwrap().message(),
        Some("failed")
    );
}

#[test]
fn invalid_opaque_union_offset_is_rejected() {
    let mut bytes = fixture("frame_unknown_op.bin");
    let operation = first_message(&bytes, &WireLimits::default())
        .operation()
        .unwrap();
    let position =
        operation._tab.loc() + usize::from(operation._tab.vtable().get(Operation::VT_UI));
    for offset in [0, u32::MAX] {
        bytes[position..position + 4].copy_from_slice(&offset.to_le_bytes());
        assert!(matches!(
            read_frame(&bytes, &WireLimits::default()),
            Err(WireError::Invalid { .. })
        ));
    }
}

#[test]
fn read_never_panics_on_truncated_fixture() {
    for_fixtures(|bytes, signed_state| {
        for length in 0..bytes.len() {
            exercise_read(&bytes[..length], signed_state);
        }
    });
}

#[test]
fn read_never_panics_on_bit_flips() {
    for_fixtures(|bytes, signed_state| {
        let mut mutated = bytes.to_vec();
        for index in 0..bytes.len().min(256) {
            for bit in 0..u8::BITS {
                mutated[index] = bytes[index] ^ (1 << bit);
                exercise_read(&mutated, signed_state);
            }
            mutated[index] = bytes[index];
        }
    });
}

fn first_message<'a>(bytes: &'a [u8], limits: &WireLimits) -> foundation_ui_wire::Message<'a> {
    read_frame(bytes, limits)
        .unwrap()
        .kind_as_frame_message()
        .unwrap()
        .messages()
        .unwrap()
        .get(0)
}

fn fixtures_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures"))
}

fn fixture(name: &str) -> Vec<u8> {
    fs::read(fixtures_dir().join(name)).unwrap()
}

fn for_fixtures(mut check: impl FnMut(&[u8], bool)) {
    let fixtures = fs::read_dir(fixtures_dir()).unwrap();
    let mut count = 0;
    for entry in fixtures {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "bin") {
            let signed_state = path.file_name().unwrap() == "state_counter.bin";
            check(&fs::read(&path).unwrap(), signed_state);
            count += 1;
        }
    }
    assert_eq!(count, 13);
}

fn exercise_read(bytes: &[u8], signed_state: bool) {
    let limits = WireLimits::default();
    if signed_state {
        let _result = read_signed_state(bytes, &limits);
    } else {
        let _result = read_frame(bytes, &limits);
    }
}

#[test]
fn wire_limits_remain_positive() {
    let defaults = WireLimits::default();
    assert_eq!(defaults.frame_bytes(), 4 * 1024 * 1024);
    assert_eq!(defaults.action_request_bytes(), 1024 * 1024);
    assert_eq!(defaults.depth(), 64);
    assert_eq!(defaults.tables(), 1_000_000);

    let limits = defaults
        .max_frame_bytes(0)
        .max_action_request_bytes(0)
        .max_depth(0)
        .max_tables(0);
    assert_eq!(limits.frame_bytes(), 1);
    assert_eq!(limits.action_request_bytes(), 1);
    assert_eq!(limits.depth(), 1);
    assert_eq!(limits.tables(), 1);
}
