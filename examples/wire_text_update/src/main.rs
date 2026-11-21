use foundation_ui_wire::{
    finish_frame, read_frame, FrameKindT, FrameMessageT, FrameT, MessageT,
    OperationT, SetTextT, UiOperationT, WireError, WireLimits, SCHEMA_VERSION,
};

fn main() -> Result<(), WireError> {
    let mut text = SetTextT::default();
    text.node_id = 42;
    text.text = Some("Hello".into());

    let mut operation = OperationT::default();
    operation.ui = UiOperationT::SetText(Box::new(text));
    let mut message = MessageT::default();
    message.operation = Some(Box::new(operation));

    let mut reply = FrameMessageT::default();
    reply.ack = 5;
    reply.messages = Some(vec![message]);

    let mut frame = FrameT::default();
    frame.schema_version = SCHEMA_VERSION;
    frame.stream_id = 7;
    frame.sequence_id = 20;
    frame.kind = FrameKindT::FrameMessage(Box::new(reply));

    let bytes = finish_frame(&frame)?;
    let view = read_frame(&bytes, &WireLimits::default())?;
    let reply = view.kind_as_frame_message().expect("built a reply");
    let first = reply.messages().expect("built one message").get(0);
    let text = first.operation().unwrap().ui_as_set_text().unwrap();
    assert_eq!((text.node_id(), text.text()), (42, Some("Hello")));
    Ok(())
}
