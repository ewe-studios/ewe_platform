use foundation_ui_wire::{
    finish_signed_state, read_frame,
    vocab::{PropKind, VocabError},
    writer::{
        columnar_batch, write_messages, ClientCaps, ColumnarError, FrameHeader, WriteError,
        WriterConfig,
    },
    AttachStateT, MessageT, OperationT, Platform, PropId, PropValueT, ReplaceSubtreeT, SetPropT,
    SetTextT, SignedStateT, TextValueT, UiOperationT, WebOperationT, WireLimits,
};

const HEADER: FrameHeader = FrameHeader {
    stream_id: 7,
    sequence_id: 20,
    ack: 5,
};

#[test]
fn wrong_prop_value_is_rejected_before_the_gate() {
    let bad_value = PropValueT::Text(Box::new(table!(TextValueT {
        v: Some("8px".into())
    })));
    let prop = UiOperationT::SetProp(Box::new(table!(SetPropT {
        node_id: 1,
        prop: PropId::PADDING,
        value: bad_value,
    })));
    let error = WriteError::Vocab {
        message_index: 1,
        error: VocabError::PropKindMismatch {
            prop: PropId::PADDING,
            expected: PropKind::Length,
            found: Some(PropKind::Text),
        },
    };
    let first = table!(MessageT {
        operation: Some(Box::new(table!(OperationT {
            web: WebOperationT::HtmlMorph(Box::default()),
        }))),
    });
    let bad = subtree(vec![table!(OperationT { ui: prop })]);
    let caps = ClientCaps {
        vocabulary_version: 99,
        ..ClientCaps::native_current(Platform::Ios)
    };
    assert_eq!(
        write_messages(vec![first, bad], &caps, HEADER, &WriterConfig::default()),
        Err(error),
    );
}

#[test]
fn empty_messages_and_operations_are_rejected() {
    let empty_messages = [
        MessageT::default(),
        table!(MessageT {
            operation: Some(Box::default())
        }),
        subtree(vec![OperationT::default()]),
    ];
    for empty in empty_messages {
        assert_eq!(
            write_messages(
                vec![message(UiOperationT::SetText(Box::default())), empty],
                &ClientCaps::web_current(),
                HEADER,
                &WriterConfig::default(),
            ),
            Err(WriteError::EmptyMessage { message_index: 1 }),
        );
    }
}

#[test]
fn attachment_requires_a_byte_vector() {
    let caps = ClientCaps::web_current();
    assert_eq!(
        write_messages(
            vec![message(attach(None))],
            &caps,
            HEADER,
            &WriterConfig::default()
        ),
        Err(WriteError::MissingState { message_index: 0 })
    );
    assert_eq!(
        columnar_batch(&[attach(None)]),
        Err(ColumnarError::InvalidValue { row: 0 })
    );
}

#[test]
fn signer_bytes_are_opaque_to_the_writer() {
    let config = WriterConfig::default();
    for bytes in [
        vec![],
        vec![0xff, 0, 0xfe],
        finish_signed_state(&SignedStateT::default()),
    ] {
        let input = vec![message(attach(Some(bytes)))];
        let (frame, _) =
            write_messages(input.clone(), &ClientCaps::web_current(), HEADER, &config).unwrap();
        assert_eq!(
            read_frame(&frame, config.wire_limits())
                .unwrap()
                .kind_as_frame_message()
                .unwrap()
                .unpack()
                .messages,
            Some(input),
        );
    }
}

#[test]
fn oversized_attachment_is_rejected_in_nested_and_columnar_forms() {
    let config = WriterConfig::default().limits(WireLimits::default().max_action_request_bytes(8));
    let row = attach(Some(vec![0; 9]));
    let batch = columnar_batch(core::slice::from_ref(&row)).unwrap();
    for operation in [
        table!(OperationT { ui: row }),
        table!(OperationT {
            columnar: Some(Box::new(batch))
        }),
    ] {
        let input = vec![
            message(UiOperationT::SetText(Box::default())),
            subtree(vec![operation]),
        ];
        assert_eq!(
            write_messages(input, &ClientCaps::web_current(), HEADER, &config),
            Err(WriteError::StateTooLarge {
                message_index: 1,
                limit: 8,
                actual: 9
            }),
        );
    }
}

#[test]
fn frame_above_limit_is_rejected() {
    let config = WriterConfig::default().limits(WireLimits::default().max_frame_bytes(1024));
    let text = table!(SetTextT {
        node_id: 42,
        text: Some("a".repeat(2000)),
    });
    assert!(matches!(
        write_messages(
            vec![message(UiOperationT::SetText(Box::new(text)))],
            &ClientCaps::web_current(),
            HEADER,
            &config,
        ),
        Err(WriteError::TooLarge { limit: 1024, actual }) if actual > 2000
    ));
}

fn message(ui: UiOperationT) -> MessageT {
    table!(MessageT {
        operation: Some(Box::new(table!(OperationT { ui: ui })))
    })
}

fn subtree(operations: Vec<OperationT>) -> MessageT {
    message(UiOperationT::ReplaceSubtree(Box::new(table!(
        ReplaceSubtreeT {
            node_id: 40,
            operations: Some(operations),
        }
    ))))
}

fn attach(state: Option<Vec<u8>>) -> UiOperationT {
    UiOperationT::AttachState(Box::new(table!(AttachStateT {
        root_node: 40,
        state: state
    })))
}
