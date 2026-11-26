use std::sync::Arc;

use foundation_ui_runtime::action;
use foundation_ui_runtime::action::{
    ActionRegistry, ActionResult, Context, Interaction, Origin, Scope, Surface,
};
use foundation_ui_runtime::dispatch::{Dispatcher, DispatcherBuilder, HelloError, HelloOutcome};
use foundation_ui_wire::vocab::VOCABULARY_VERSION;
use foundation_ui_wire::writer::{WriteError, WriterConfig};
use foundation_ui_wire::{
    finish_frame, read_frame, CapsStatus, ClientHelloT, FrameKindName, FrameKindT, FrameRequestT,
    FrameT, Platform, ServerCapsT, WireError, WireLimits, SCHEMA_VERSION,
};

use super::support::{test_ctx, WALL};

#[action(server)]
async fn increment_counter(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

#[action]
async fn alpha(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

#[action(server, mobile)]
async fn mobile_list(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

#[action(host)]
async fn host_only(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

fn dispatcher(config: WriterConfig) -> Dispatcher {
    let mut registry = ActionRegistry::new();
    registry.register(mobile_list).unwrap();
    registry.register(increment_counter).unwrap();
    registry.register(alpha).unwrap();
    registry.register(host_only).unwrap();
    registry.register_page("page").unwrap();
    registry
        .declare(Some(Origin::Server), "absent", Scope::All)
        .unwrap();
    DispatcherBuilder::new(Origin::Server, Arc::new(registry))
        .writer_config(config)
        .build()
}

fn hello(
    schema: u16,
    vocabulary: u32,
    mount: Surface,
    stream_id: u64,
    resume_from: u64,
) -> Vec<u8> {
    let mut input = ClientHelloT::default();
    input.mount = mount;
    input.platform = if mount == Surface::Web {
        Platform::Web
    } else {
        Platform::Ios
    };
    input.vocabulary_version = vocabulary;
    input.resume_from = resume_from;
    let mut request = FrameRequestT::default();
    request.hello = Some(Box::new(input));
    let mut frame = FrameT::default();
    frame.schema_version = schema;
    frame.stream_id = stream_id;
    frame.kind = FrameKindT::FrameRequest(Box::new(request));
    finish_frame(&frame).unwrap()
}

fn read_caps(bytes: &[u8]) -> ServerCapsT {
    let frame = read_frame(bytes, &WireLimits::default()).unwrap();
    assert_eq!(frame.schema_version(), SCHEMA_VERSION);
    let message = frame.kind_as_frame_message().unwrap();
    assert_eq!((frame.sequence_id(), message.ack()), (0, 0));
    let messages = message.messages().unwrap();
    assert_eq!(messages.len(), 1);
    let caps = messages.get(0).control().unwrap().caps().unwrap().unpack();
    assert_eq!(frame.stream_id(), caps.stream_id);
    caps
}

#[test]
fn caps_actions_are_sorted_and_hashed() {
    let dispatch = dispatcher(WriterConfig::default());
    let (ctx, _) = test_ctx(WALL);
    let bytes = hello(SCHEMA_VERSION, VOCABULARY_VERSION, Surface::Native, 7, 19);
    let HelloOutcome::Accepted {
        connection,
        caps_frame,
    } = dispatch.hello(&ctx, &bytes).unwrap()
    else {
        panic!("compatible hello must create a connection")
    };
    assert_eq!(connection.surface(), Surface::Native);
    assert_eq!(connection.caps().platform, Platform::Ios);
    let caps = read_caps(&caps_frame);
    assert_eq!(caps.status, CapsStatus::Ok);
    assert_eq!((caps.stream_id, caps.resume_from), (7, 19));
    let actions: Vec<_> = caps
        .actions
        .unwrap()
        .into_iter()
        .map(|entry| (entry.name.unwrap(), entry.hash))
        .collect();
    assert_eq!(
        actions,
        [
            ("alpha".to_owned(), 0x5716_bfae_8da5_bf03),
            ("increment_counter".to_owned(), 0x4ea9_737a_00bf_1f19),
            ("mobile_list".to_owned(), 0xe17b_a0d2_b71f_5864),
        ]
    );
}

#[test]
fn supported_older_version_uses_seeded_stream_id() {
    let dispatch = dispatcher(WriterConfig::default());
    let bytes = hello(SCHEMA_VERSION, 1, Surface::Web, 0, 25);
    let start = || {
        let (ctx, _) = test_ctx(WALL);
        let HelloOutcome::Accepted {
            connection,
            caps_frame,
        } = dispatch.hello(&ctx, &bytes).unwrap()
        else {
            panic!("vocabulary one must be supported")
        };
        let caps = read_caps(&caps_frame);
        assert_ne!(connection.stream_id(), 0);
        assert_eq!(caps.vocabulary_version, 1);
        assert_eq!(caps.resume_from, 0);
        assert_eq!(connection.caps().vocabulary_version, 1);
        assert_eq!(caps.actions.unwrap().len(), 2);
        connection.stream_id()
    };
    assert_eq!(start(), start());
}

#[test]
fn version_mismatch_advertises_server_versions_and_actions() {
    let dispatch = dispatcher(WriterConfig::default());
    let (ctx, _) = test_ctx(WALL);
    for (schema, vocabulary) in [
        (SCHEMA_VERSION + 1, VOCABULARY_VERSION),
        (SCHEMA_VERSION, VOCABULARY_VERSION + 1),
    ] {
        let bytes = hello(schema, vocabulary, Surface::Web, 3, 4);
        let HelloOutcome::UpdateRequired { caps_frame } = dispatch.hello(&ctx, &bytes).unwrap()
        else {
            panic!("incompatible hello must request an update")
        };
        let caps = read_caps(&caps_frame);
        assert_eq!(caps.status, CapsStatus::UpdateRequired);
        assert_eq!(
            (caps.schema_version, caps.vocabulary_version),
            (SCHEMA_VERSION, VOCABULARY_VERSION)
        );
        assert_eq!((caps.stream_id, caps.resume_from), (0, 0));
        assert_eq!(caps.actions.unwrap(), dispatch.server_actions(Surface::Web));
    }
}

#[test]
fn malformed_input_and_reply_limits_have_typed_errors() {
    let dispatch = dispatcher(WriterConfig::default());
    let (ctx, _) = test_ctx(WALL);
    assert!(matches!(
        dispatch.hello(&ctx, b""),
        Err(HelloError::Wire(WireError::TooShort { .. }))
    ));
    let bytes = hello(SCHEMA_VERSION, VOCABULARY_VERSION, Surface::Web, 7, 0);
    let HelloOutcome::Accepted { caps_frame, .. } = dispatch.hello(&ctx, &bytes).unwrap() else {
        panic!("compatible hello must be accepted")
    };
    assert_eq!(
        dispatch.hello(&ctx, &caps_frame).unwrap_err(),
        HelloError::Wire(WireError::WrongKind {
            expected: FrameKindName::Request,
            found: FrameKindName::Message
        })
    );
    let action = super::dispatch_support::request("server:alpha", 40, None, 1);
    assert_eq!(
        dispatch.hello(&ctx, &action).unwrap_err(),
        HelloError::NotAHello
    );
    let exhausted = hello(
        SCHEMA_VERSION,
        VOCABULARY_VERSION,
        Surface::Web,
        7,
        u64::MAX,
    );
    assert_eq!(
        dispatch.hello(&ctx, &exhausted).unwrap_err(),
        HelloError::InvalidResume
    );
    let limited = dispatcher(
        WriterConfig::default().limits(WireLimits::default().max_frame_bytes(bytes.len())),
    );
    assert!(matches!(
        limited.hello(&ctx, &bytes),
        Err(HelloError::Write(WriteError::TooLarge { .. }))
    ));
}

#[foundation_core::valtron::valtron_test]
async fn resumed_connection_uses_separate_request_and_reply_sequences() {
    let dispatch = dispatcher(WriterConfig::default());
    let (ctx, _) = test_ctx(WALL);
    let bytes = hello(SCHEMA_VERSION, VOCABULARY_VERSION, Surface::Web, 7, 19);
    let HelloOutcome::Accepted { connection, .. } = dispatch.hello(&ctx, &bytes).unwrap() else {
        panic!("resume must return a connection");
    };
    let request = super::dispatch_support::request("server:alpha", 42, None, 1);
    let reply = dispatch.dispatch(&ctx, &connection, &request).await;
    let frame = read_frame(&reply, &WireLimits::default()).unwrap();
    let message = frame.kind_as_frame_message().unwrap();
    assert_eq!(
        (frame.stream_id(), frame.sequence_id(), message.ack()),
        (7, 20, 1)
    );
    assert_eq!(message.messages().unwrap().len(), 0);
}
