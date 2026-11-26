use foundation_ui_wire::{
    read_frame,
    writer::{
        columnar_batch, columnar_rows, write_messages, ClientCaps, StreamSequencer, WriterConfig,
    },
    MessageT, OperationT, Platform, SetTextT, UiOperationT,
};

fn main() {
    let caps = ClientCaps::web_current();
    let config = WriterConfig::default();
    let mut sequence = StreamSequencer::resume(7, 19);

    let mut operation = OperationT::default();
    operation.ui = set_text(42, "Ready");
    let (bytes, report) =
        write_messages(vec![message(operation)], &caps, sequence.header(5), &config)
            .expect("write the text reply");
    assert_eq!(report.filtered, []);

    let frame = read_frame(&bytes, config.wire_limits()).expect("verify the frame");
    assert_eq!((frame.stream_id(), frame.sequence_id()), (7, 20));
    let reply = frame
        .kind_as_frame_message()
        .expect("the writer makes a reply");
    assert_eq!(reply.ack(), 5);
    let text = reply
        .messages()
        .expect("one message")
        .get(0)
        .operation()
        .expect("an operation")
        .ui_as_set_text()
        .expect("a text update");
    assert_eq!((text.node_id(), text.text()), (42, Some("Ready")));

    let rows = [set_text(1, "a"), set_text(2, "bb"), set_text(3, "ccc")];
    let mut operation = OperationT::default();
    operation.columnar = Some(Box::new(
        columnar_batch(&rows).expect("build the columnar batch"),
    ));
    let native = ClientCaps::native_current(Platform::Ios);
    let (bytes, report) = write_messages(
        vec![message(operation)],
        &native,
        sequence.header(0),
        &config,
    )
    .expect("write the columnar push");
    assert_eq!(report.filtered, []);

    let frame = read_frame(&bytes, config.wire_limits()).expect("verify the frame");
    assert_eq!((frame.stream_id(), frame.sequence_id()), (7, 21));
    let push = frame
        .kind_as_frame_message()
        .expect("the writer makes a push");
    assert_eq!(push.ack(), 0);
    let batch = push
        .messages()
        .expect("one message")
        .get(0)
        .operation()
        .expect("an operation")
        .columnar()
        .expect("a columnar batch");
    for column in [
        batch.node_ids(),
        batch.arg_a(),
        batch.arg_b(),
        batch.blob_a_off(),
        batch.blob_b_off(),
    ] {
        let offset =
            column.expect("a numeric column").bytes().as_ptr().addr() - bytes.as_ptr().addr();
        assert_eq!(offset % 8, 0);
    }
    assert_eq!(
        columnar_rows(batch).expect("decode the columnar rows"),
        rows
    );
    println!(
        "Wrote a text reply and an aligned batch of {} rows.",
        rows.len()
    );
}

fn set_text(node_id: u32, value: &str) -> UiOperationT {
    let mut text = SetTextT::default();
    text.node_id = node_id;
    text.text = Some(value.into());
    UiOperationT::SetText(Box::new(text))
}

fn message(operation: OperationT) -> MessageT {
    let mut message = MessageT::default();
    message.operation = Some(Box::new(operation));
    message
}
