use foundation_ui_wire::{
    finish_frame, finish_signed_state, read_frame, read_signed_state, ActionInputT, ActionRequestT,
    FrameKindT, FrameMessageT, FrameRequestT, FrameT, MessageT, OperationT, SetTextT, SignedStateT,
    UiOperationT, WireLimits, FRAME_IDENTIFIER, SCHEMA_VERSION, STATE_IDENTIFIER,
};

use super::fixtures_tests::all_forms_frame;

#[test]
fn frame_round_trip_keeps_every_field() {
    let text = table!(SetTextT {
        node_id: 42,
        text: Some("1".into()),
    });
    let operation = table!(OperationT {
        ui: UiOperationT::SetText(Box::new(text)),
    });
    let message = table!(MessageT {
        operation: Some(Box::new(operation)),
    });
    let frame_body = table!(FrameMessageT {
        subscription_id: 0,
        ack: 5,
        messages: Some(vec![message]),
    });
    let frame = table!(FrameT {
        schema_version: SCHEMA_VERSION,
        stream_id: 7,
        sequence_id: 20,
        kind: FrameKindT::FrameMessage(Box::new(frame_body)),
    });
    assert_frame_round_trip(&frame);
}

#[test]
fn request_frame_round_trip() {
    let state = finish_signed_state(&state_value());
    let input = table!(ActionInputT {
        event_name: Some("tap".into()),
        value: Some("1".into()),
        checked: true,
        key_code: 13,
        modifiers: 1,
        parent_node: 40,
    });
    let action = table!(ActionRequestT {
        action: Some("server:increment_counter".into()),
        node_id: 42,
        input: Some(Box::new(input)),
        state: Some(state),
    });
    let request = table!(FrameRequestT {
        action: Some(Box::new(action)),
    });
    let frame = table!(FrameT {
        schema_version: SCHEMA_VERSION,
        stream_id: 7,
        sequence_id: 5,
        kind: FrameKindT::FrameRequest(Box::new(request)),
    });
    assert_frame_round_trip(&frame);
}

#[test]
fn signed_state_round_trip() {
    let state = state_value();
    let bytes = finish_signed_state(&state);
    assert_eq!(&bytes[4..8], STATE_IDENTIFIER.as_bytes());
    assert_eq!(
        read_signed_state(&bytes, &WireLimits::default())
            .unwrap()
            .unpack(),
        state
    );
}

#[test]
fn all_operation_forms_round_trip() {
    let frame = all_forms_frame();
    let FrameKindT::FrameMessage(message) = &frame.kind else {
        panic!("Expected a message frame");
    };
    assert_eq!(message.messages.as_ref().unwrap().len(), 31);
    assert_frame_round_trip(&frame);
}

fn assert_frame_round_trip(frame: &FrameT) {
    let bytes = finish_frame(frame).unwrap();
    assert_eq!(&bytes[4..8], FRAME_IDENTIFIER.as_bytes());
    assert_eq!(
        &read_frame(&bytes, &WireLimits::default()).unwrap().unpack(),
        frame
    );
}

fn state_value() -> SignedStateT {
    table!(SignedStateT {
        format_version: 1,
        flags: 2,
        key_id: 1,
        issued_at: 1000,
        expires_at: 87_400,
        component_type: 7,
        root_node: 40,
        session_binding: Some(vec![0x11; 32]),
        body: Some(vec![1, 0, 0, 0]),
        tag: Some(vec![0x22; 32]),
    })
}

#[test]
fn subscription_round_trip_preserves_the_original_update_and_cursor() {
    use foundation_ui_wire::subscription::{replay_expired, resume_subscription, tag_replay};
    use std::num::NonZeroU64;
    let session = NonZeroU64::new(13).unwrap();
    let listener = NonZeroU64::new(42).unwrap();
    let stream = NonZeroU64::new(7).unwrap();
    let limits = WireLimits::default();
    let query = resume_subscription(session, listener, stream, 12).unwrap();
    let resume = read_frame(&query, &limits)
        .unwrap()
        .kind_as_frame_request()
        .unwrap()
        .resume()
        .unwrap();
    assert_eq!(
        (
            resume.delivery_session(),
            resume.subscription_id(),
            resume.stream_id(),
            resume.after()
        ),
        (13, 42, 7, 12)
    );
    assert!(tag_replay(&query, listener, &limits).is_err());
    let expired = replay_expired(listener, stream, 12).unwrap();
    let msg = read_frame(&expired, &limits)
        .unwrap()
        .kind_as_frame_message()
        .unwrap();
    assert_eq!(msg.subscription_id(), 42);
    assert_eq!(
        msg.messages()
            .unwrap()
            .get(0)
            .control()
            .unwrap()
            .replay_expired()
            .unwrap()
            .after(),
        12
    );
    let mut original = FrameT::default();
    original.schema_version = SCHEMA_VERSION;
    original.stream_id = 7;
    original.sequence_id = 13;
    original.kind = FrameKindT::FrameMessage(Box::default());
    let broadcast = finish_frame(&original).unwrap();
    let targeted = tag_replay(&broadcast, listener, &limits).unwrap();
    assert_eq!(
        read_frame(&broadcast, &limits)
            .unwrap()
            .kind_as_frame_message()
            .unwrap()
            .subscription_id(),
        0
    );
    let tagged = read_frame(&targeted, &limits).unwrap();
    assert_eq!(tagged.sequence_id(), 13);
    assert_eq!(
        tagged.kind_as_frame_message().unwrap().subscription_id(),
        42
    );
    assert!(tag_replay(&targeted, listener, &limits).is_err());
}
