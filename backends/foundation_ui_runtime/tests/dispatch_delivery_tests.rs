use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_ui_runtime::action::{
    ActionRegistry, ActionResult, Context, Interaction, Origin, Surface,
};
use foundation_ui_runtime::dispatch::{
    Connection, DefaultDelivery, DeliveryHandler, DispatcherBuilder,
};
use foundation_ui_runtime::state::{
    install_state_signer, verify_state, HmacStateSigner, StateConfig,
};
use foundation_ui_wire::writer::{ClientCaps, FrameHeader, WriteError};
use foundation_ui_wire::{
    read_frame, ErrorCode, HtmlMorphT, MessageT, OperationT, Platform, SetTextT, TargetSelectorT,
    UiOperationT, WebOperation, WebOperationT, WireLimits,
};

use super::dispatch_support::request;
use super::support::{test_ctx, WALL};

#[foundation_ui_runtime::action(server)]
async fn render(ctx: &Context, event: Interaction<()>) -> ActionResult {
    let mut text = SetTextT::default();
    text.node_id = event.node_id;
    text.text = Some("x".into());
    let mut target = TargetSelectorT::default();
    target.node_id = 40;
    let mut morph = HtmlMorphT::default();
    morph.target = Some(Box::new(target));
    morph.content = Some("<b>x</b>".into());
    let mut operation = OperationT::default();
    operation.ui = UiOperationT::SetText(Box::new(text));
    operation.web = WebOperationT::HtmlMorph(Box::new(morph));
    ctx.receiver().render_op(operation);
    ctx.receiver().data(event.node_id, vec![0x91, 0x01]);
    ctx.receiver().state_for(&[40, event.node_id], &2u32)
}

fn builder() -> DispatcherBuilder {
    let mut registry = ActionRegistry::new();
    registry.register(render).unwrap();
    DispatcherBuilder::new(Origin::Server, Arc::new(registry))
}

struct RecordingDelivery {
    calls: AtomicUsize,
    fail: bool,
}

impl DeliveryHandler for RecordingDelivery {
    fn messages(
        &self,
        messages: Vec<MessageT>,
        caps: &ClientCaps,
        header: FrameHeader,
    ) -> Result<Vec<u8>, WriteError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        assert_eq!(messages.len(), 4);
        if self.fail {
            return Err(WriteError::TooLarge {
                limit: 4,
                actual: 5,
            });
        }
        DefaultDelivery::default().messages(messages, caps, header)
    }
}

#[valtron_test]
async fn custom_delivery_encodes_signed_messages_in_receiver_order_for_a_native_surface() {
    let (ctx, _) = test_ctx(WALL);
    install_state_signer(
        &ctx,
        Arc::new(HmacStateSigner::new([11; 32], StateConfig::default()).unwrap()),
    );
    let delivery = Arc::new(RecordingDelivery {
        calls: AtomicUsize::new(0),
        fail: false,
    });
    let dispatcher = builder().delivery(delivery.clone()).build();
    let connection = Connection::stateless(
        ClientCaps::native_current(Platform::Ios),
        Surface::Native,
        7,
    );
    let answer = dispatcher
        .dispatch(&ctx, &connection, &request("server:render", 42, None, 10))
        .await;
    assert_eq!(delivery.calls.load(Ordering::Relaxed), 1);
    let frame = read_frame(&answer, &WireLimits::default()).unwrap();
    assert_eq!(frame.stream_id(), 7);
    let body = frame.kind_as_frame_message().unwrap();
    assert_eq!(body.ack(), 10);
    let messages = body.messages().unwrap();
    assert_eq!(messages.len(), 4);
    let op = messages.get(0).operation().unwrap();
    assert_eq!(op.web_type(), WebOperation::NONE);
    let text = op.ui_as_set_text().unwrap();
    assert_eq!((text.node_id(), text.text()), (42, Some("x")));
    let data = messages.get(1).data().unwrap();
    assert_eq!(data.target_node(), 42);
    let blob = data.msgpack().unwrap();
    assert_eq!(blob.target_node(), 42);
    assert_eq!(blob.bytes().unwrap().bytes(), &[0x91, 0x01]);
    let root = messages
        .get(2)
        .operation()
        .unwrap()
        .ui_as_attach_state()
        .unwrap();
    let node = messages
        .get(3)
        .operation()
        .unwrap()
        .ui_as_attach_state()
        .unwrap();
    assert_eq!((root.root_node(), node.root_node()), (40, 42));
    let attachment = node.state().unwrap().bytes();
    assert_eq!(root.state().unwrap().bytes(), attachment);
    assert_eq!(verify_state(&ctx, attachment).unwrap(), b"2");
}

#[valtron_test]
async fn failed_delivery_returns_one_error_without_any_queued_output() {
    let (ctx, _) = test_ctx(WALL);
    install_state_signer(
        &ctx,
        Arc::new(HmacStateSigner::new([11; 32], StateConfig::default()).unwrap()),
    );
    let delivery = Arc::new(RecordingDelivery {
        calls: AtomicUsize::new(0),
        fail: true,
    });
    let dispatcher = builder().delivery(delivery.clone()).build();
    let connection = Connection::stateless(ClientCaps::web_current(), Surface::Web, 7);
    let answer = dispatcher
        .dispatch(&ctx, &connection, &request("server:render", 42, None, 17))
        .await;
    assert_eq!(delivery.calls.load(Ordering::Relaxed), 1);
    let frame = read_frame(&answer, &WireLimits::default()).unwrap();
    let body = frame.kind_as_frame_message().unwrap();
    assert_eq!(body.ack(), 17);
    let messages = body.messages().unwrap();
    assert_eq!(messages.len(), 1);
    let message = messages.get(0);
    assert!(message.operation().is_none());
    assert!(message.data().is_none());
    assert_eq!(
        message.control().unwrap().error().unwrap().code(),
        ErrorCode::ActionFailed
    );
}
