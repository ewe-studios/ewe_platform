use foundation_ui_wire::{
    finish_signed_state, read_frame,
    vocab::{PropKind, VocabError},
    writer::{write_messages, ClientCaps, FrameHeader, WriteError, WriterConfig},
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
fn unsealed_state_is_rejected() {
    let caps = ClientCaps::web_current();
    let config = WriterConfig::default();
    for bytes in [
        None,
        Some(finish_signed_state(&SignedStateT::default())),
        Some(finish_signed_state(&table!(SignedStateT {
            tag: Some(vec![])
        }))),
    ] {
        let input = vec![message(attach(bytes))];
        assert_eq!(
            write_messages(input, &caps, HEADER, &config),
            Err(WriteError::UnsealedState { message_index: 0 }),
        );
    }
}

#[test]
fn unsealed_state_is_allowed_by_config() {
    let config = WriterConfig::default().allow_unsealed_state(true);
    let state = finish_signed_state(&table!(SignedStateT {
        root_node: 40,
        body: Some(vec![1, 2])
    }));
    let input = vec![message(attach(Some(state)))];
    let (frame, report) =
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
    assert_eq!(report.filtered, []);
}

#[test]
fn invalid_or_large_state_fails_when_unsealed_state_is_allowed() {
    let caps = ClientCaps::web_current();
    let config = WriterConfig::default().allow_unsealed_state(true);
    let state = finish_signed_state(&table!(SignedStateT {
        root_node: 40,
        body: Some(vec![1, 2]),
        tag: Some(vec![3]),
    }));
    let examples = [
        (vec![1, 2, 3], config.clone()),
        (
            state,
            config.limits(WireLimits::default().max_action_request_bytes(8)),
        ),
    ];
    for (state, config) in examples {
        let input = vec![
            message(UiOperationT::SetText(Box::default())),
            subtree(vec![table!(OperationT {
                ui: attach(Some(state))
            })]),
        ];
        assert_eq!(
            write_messages(input, &caps, HEADER, &config),
            Err(WriteError::UnsealedState { message_index: 1 }),
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
