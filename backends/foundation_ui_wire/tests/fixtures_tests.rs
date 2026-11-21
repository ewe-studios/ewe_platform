use std::{env, fs, path::Path};

use flatbuffers::FlatBufferBuilder;
use foundation_ui_wire::*;

fn message(operation: OperationT) -> MessageT {
    table!(MessageT {
        operation: Some(Box::new(operation))
    })
}

fn message_frame(messages: Vec<MessageT>) -> FrameT {
    table!(FrameT {
        schema_version: 1,
        stream_id: 7,
        sequence_id: 20,
        kind: FrameKindT::FrameMessage(Box::new(table!(FrameMessageT {
            ack: 5,
            messages: Some(messages),
        }))),
    })
}

fn request_frame(stream_id: u64, sequence_id: u64, request: FrameRequestT) -> FrameT {
    table!(FrameT {
        schema_version: 1,
        stream_id: stream_id,
        sequence_id: sequence_id,
        kind: FrameKindT::FrameRequest(Box::new(request)),
    })
}

fn control_frame(control: ControlT) -> FrameT {
    message_frame(vec![table!(MessageT {
        control: Some(Box::new(control))
    })])
}

fn counter_frame() -> FrameT {
    message_frame(vec![message(table!(OperationT {
        ui: UiOperationT::SetText(Box::new(table!(SetTextT {
            node_id: 42,
            text: Some("1".into()),
        }))),
    }))])
}

fn shared_forms() -> [UiOperationT; 12] {
    use UiOperationT as Ui;
    [
        Ui::CreateNode(Box::new(table!(CreateNodeT {
            node_id: 1,
            kind: NodeKind::TEXT
        }))),
        Ui::SetText(Box::new(table!(SetTextT {
            node_id: 1,
            text: Some("a".into())
        }))),
        Ui::SetProp(Box::new(table!(SetPropT {
            node_id: 1,
            prop: PropId::ENABLED,
            value: PropValueT::Bool(Box::new(table!(BoolValueT { v: true }))),
        }))),
        Ui::AppendChild(Box::new(table!(AppendChildT {
            parent_id: 1,
            child_id: 2
        }))),
        Ui::InsertBefore(Box::new(table!(InsertBeforeT {
            parent_id: 1,
            child_id: 2,
            before_id: 3
        }))),
        Ui::Remove(Box::new(table!(RemoveT { node_id: 2 }))),
        Ui::ReplaceSubtree(Box::new(table!(ReplaceSubtreeT {
            node_id: 1,
            operations: Some(vec![])
        }))),
        Ui::BindAction(Box::new(table!(BindActionT {
            node_id: 1,
            event: EventId::TAP,
            action: Some("server:increment_counter".into())
        }))),
        Ui::UnbindAction(Box::new(table!(UnbindActionT {
            node_id: 1,
            event: EventId::TAP
        }))),
        Ui::AttachState(Box::new(table!(AttachStateT {
            root_node: 40,
            state: Some(state_bytes())
        }))),
        Ui::LiveMount(Box::new(table!(LiveMountT {
            node_id: 1,
            channel_id: 1
        }))),
        Ui::UiBehaviour(Box::new(table!(UiBehaviourT {
            node_id: 1,
            behaviour: BehaviourId::DIALOG
        }))),
    ]
}

fn web_forms() -> [WebOperationT; 16] {
    use WebOperationT as Web;
    [
        Web::WebElement(Box::new(table!(WebElementT {
            node_id: 1,
            custom_tag: Some("div".into())
        }))),
        Web::WebText(Box::new(table!(WebTextT {
            node_id: 1,
            content: Some("a".into())
        }))),
        Web::WebSetAttribute(Box::new(table!(WebSetAttributeT {
            node_id: 1,
            custom_attr: Some("title".into()),
            value: Some("a".into())
        }))),
        Web::WebRemoveAttribute(Box::new(table!(WebRemoveAttributeT {
            node_id: 1,
            custom_attr: Some("title".into())
        }))),
        Web::WebProperty(Box::new(table!(WebPropertyT {
            node_id: 1,
            custom_name: Some("title".into()),
            value: Some("a".into())
        }))),
        Web::WebAddListener(Box::new(table!(WebAddListenerT {
            node_id: 1,
            custom_name: Some("click".into())
        }))),
        Web::WebRemoveListener(Box::new(table!(WebRemoveListenerT {
            node_id: 1,
            custom_name: Some("click".into())
        }))),
        Web::WebReplaceNode(Box::new(table!(WebReplaceNodeT {
            old_id: 1,
            new_id: 2
        }))),
        Web::WebStyle(Box::new(table!(WebStyleT {
            node_id: 1,
            custom_name: Some("display".into()),
            value: Some("none".into())
        }))),
        Web::WebAddClass(Box::new(table!(WebAddClassT {
            node_id: 1,
            class_name: Some("a".into())
        }))),
        Web::WebRemoveClass(Box::new(table!(WebRemoveClassT {
            node_id: 1,
            class_name: Some("a".into())
        }))),
        Web::HtmlMorph(Box::new(table!(HtmlMorphT {
            target: Some(Box::new(table!(TargetSelectorT { node_id: 1 }))),
            content: Some("<b>a</b>".into()),
        }))),
        Web::WebRegisterNode(Box::new(table!(WebRegisterNodeT { node_id: 1 }))),
        Web::WebUnregisterNode(Box::new(table!(WebUnregisterNodeT { node_id: 1 }))),
        Web::ScriptNode(Box::new(table!(ScriptNodeT {
            node_id: 2,
            parent_id: 1,
            body: Some("void 0".into())
        }))),
        Web::WebBehaviour(Box::new(table!(WebBehaviourT {
            node_id: 1,
            custom: Some("a".into())
        }))),
    ]
}

fn mobile_forms() -> [MobileOperationT; 3] {
    use MobileOperationT as Mobile;
    [
        Mobile::MountWebView(Box::new(table!(MountWebViewT {
            node_id: 2,
            parent_id: 1,
            url: Some("https://example.com".into())
        }))),
        Mobile::NativeCommand(Box::new(table!(NativeCommandT {
            command: NativeCommandId::HAPTIC
        }))),
        Mobile::MobileBehaviour(Box::new(table!(MobileBehaviourT {
            node_id: 1,
            custom: Some("a".into())
        }))),
    ]
}

pub(super) fn all_forms_frame() -> FrameT {
    let messages = shared_forms()
        .into_iter()
        .map(|ui| message(table!(OperationT { ui: ui })))
        .chain(
            web_forms()
                .into_iter()
                .map(|web| message(table!(OperationT { web: web }))),
        )
        .chain(
            mobile_forms()
                .into_iter()
                .map(|mobile| message(table!(OperationT { mobile: mobile }))),
        )
        .collect();
    message_frame(messages)
}

pub(super) fn columnar_bytes(second_blob_start: u32) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let op_codes = builder.create_vector(&[UiOperation::SetText.0; 3]);
    let node_ids = create_aligned_vector(&mut builder, &[1, 2, 3]);
    let arg_a = create_aligned_vector(&mut builder, &[0; 3]);
    let arg_b = create_aligned_vector(&mut builder, &[0; 3]);
    let text_offsets = create_aligned_vector(&mut builder, &[0, 1, 3, 6]);
    let empty_offsets = create_aligned_vector(&mut builder, &[second_blob_start; 4]);
    let blob_bytes = builder.create_vector(b"abbccc");
    let batch = ColumnarBatch::create(
        &mut builder,
        &ColumnarBatchArgs {
            op_codes: Some(op_codes),
            node_ids: Some(node_ids),
            arg_a: Some(arg_a),
            arg_b: Some(arg_b),
            blob_a_off: Some(text_offsets),
            blob_b_off: Some(empty_offsets),
            blob_bytes: Some(blob_bytes),
        },
    );
    let operation = Operation::create(
        &mut builder,
        &OperationArgs {
            columnar: Some(batch),
            ..Default::default()
        },
    );
    let message = Message::create(
        &mut builder,
        &MessageArgs {
            operation: Some(operation),
            ..Default::default()
        },
    );
    let messages = builder.create_vector(&[message]);
    let kind = FrameMessage::create(
        &mut builder,
        &FrameMessageArgs {
            ack: 5,
            messages: Some(messages),
        },
    );
    let frame = Frame::create(
        &mut builder,
        &FrameArgs {
            schema_version: 1,
            stream_id: 7,
            sequence_id: 20,
            kind_type: FrameKind::FrameMessage,
            kind: Some(kind.as_union_value()),
        },
    );
    builder.finish(frame, Some(FRAME_IDENTIFIER));
    builder.finished_data().to_vec()
}

fn data_frame() -> FrameT {
    let columns = vec![
        table!(ColumnT {
            name: Some("n".into()),
            data: ColumnDataT::I64(Box::new(table!(I64sT {
                values: Some(vec![1, 2])
            }))),
        }),
        table!(ColumnT {
            name: Some("x".into()),
            data: ColumnDataT::F64(Box::new(table!(F64sT {
                values: Some(vec![0.5, 1.5])
            }))),
        }),
    ];
    let data = table!(DataT {
        table: Some(Box::new(table!(DataTableT {
            target_node: 9,
            rows: 2,
            columns: Some(columns),
        }))),
    });
    message_frame(vec![table!(MessageT {
        data: Some(Box::new(data))
    })])
}

fn state_bytes() -> Vec<u8> {
    finish_signed_state(&table!(SignedStateT {
        format_version: 1,
        key_id: 1,
        issued_at: 1000,
        expires_at: 87400,
        component_type: 7,
        root_node: 40,
        session_binding: Some(vec![0x11; 32]),
        body: Some(vec![1, 0, 0, 0]),
        tag: Some(vec![0x22; 32]),
    }))
}

fn unknown_operation_bytes(counter: &[u8]) -> Vec<u8> {
    let frame = read_frame(counter, &WireLimits::default()).unwrap();
    let operation = frame
        .kind_as_frame_message()
        .unwrap()
        .messages()
        .unwrap()
        .get(0)
        .operation()
        .unwrap();
    let table = operation._tab;
    let offset = table.vtable().get(Operation::VT_UI_TYPE);
    assert_ne!(offset, 0);
    let index = table.loc() + usize::from(offset);
    assert_eq!(counter[index], UiOperation::SetText.0);
    let mut bytes = counter.to_vec();
    bytes[index] = 250;
    bytes
}

fn counter_action_bytes(state: Vec<u8>) -> Vec<u8> {
    finish_frame(&request_frame(
        7,
        5,
        table!(FrameRequestT {
            action: Some(Box::new(table!(ActionRequestT {
                action: Some("server:increment_counter".into()),
                node_id: 42,
                input: Some(Box::new(table!(ActionInputT {
                    event_name: Some("tap".into())
                }))),
                state: Some(state),
            }))),
        }),
    ))
    .unwrap()
}

fn custom_attachment_bytes(state: &[u8]) -> Vec<u8> {
    let messages = [40, 42]
        .into_iter()
        .map(|root| {
            message(table!(OperationT {
                ui: UiOperationT::AttachState(Box::new(table!(AttachStateT {
                    root_node: root,
                    state: Some(state.to_vec()),
                }))),
            }))
        })
        .collect();
    writer::write_messages(
        messages,
        &writer::ClientCaps::web_current(),
        writer::FrameHeader {
            stream_id: 7,
            sequence_id: 20,
            ack: 5,
        },
        &writer::WriterConfig::default(),
    )
    .unwrap()
    .0
}

#[test]
fn fixtures_match_built_values() {
    let counter = finish_frame(&counter_frame()).unwrap();
    let unknown = unknown_operation_bytes(&counter);
    let state = state_bytes();
    let custom_state = include_bytes!("../fixtures/state_counter_custom.json");
    let frames = [
        ("frame_ops_counter.bin", counter),
        (
            "frame_ops_all_forms.bin",
            finish_frame(&all_forms_frame()).unwrap(),
        ),
        ("frame_columnar_3rows.bin", columnar_bytes(6)),
        ("frame_data_table.bin", finish_frame(&data_frame()).unwrap()),
        (
            "frame_error.bin",
            finish_frame(&control_frame(table!(ControlT {
                error: Some(Box::new(table!(ActionErrorT {
                    code: ErrorCode::ActionFailed,
                    message: Some("failed".into())
                }))),
            })))
            .unwrap(),
        ),
        (
            "frame_state_expired.bin",
            finish_frame(&control_frame(table!(ControlT {
                state_expired: Some(Box::new(table!(StateExpiredT {
                    token_issued_at: 1_700_000_000
                }))),
            })))
            .unwrap(),
        ),
        ("frame_unknown_op.bin", unknown),
        ("action_increment.bin", counter_action_bytes(state.clone())),
        (
            "action_structured.bin",
            finish_frame(&request_frame(
                7,
                6,
                table!(FrameRequestT {
                    action: Some(Box::new(table!(ActionRequestT {
                        action: Some("server:add_todo".into()),
                        node_id: 42,
                        input: Some(Box::new(table!(ActionInputT {
                            event_name: Some("submit".into()),
                            json: Some(br#"{"list":{"id":7},"title":"Buy milk"}"#.to_vec()),
                        }))),
                    }))),
                }),
            ))
            .unwrap(),
        ),
        (
            "action_increment_custom.bin",
            counter_action_bytes(custom_state.to_vec()),
        ),
        (
            "frame_state_custom.bin",
            custom_attachment_bytes(custom_state),
        ),
        (
            "hello_web.bin",
            finish_frame(&request_frame(
                0,
                1,
                table!(FrameRequestT {
                    hello: Some(Box::new(table!(ClientHelloT {
                        vocabulary_version: 2,
                        platform: Platform::Web,
                        mount: MountSurface::Web
                    }))),
                }),
            ))
            .unwrap(),
        ),
        (
            "caps_actions.bin",
            finish_frame(&control_frame(table!(ControlT {
                caps: Some(Box::new(table!(ServerCapsT {
                    schema_version: 1,
                    vocabulary_version: 2,
                    actions: Some(vec![table!(ServerActionT {
                        hash: 0x0123_4567_89ab_cdef,
                        name: Some("increment_counter".into())
                    })]),
                }))),
            })))
            .unwrap(),
        ),
    ];
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    for (name, bytes) in frames.into_iter().chain([("state_counter.bin", state)]) {
        let path = fixtures.join(name);
        if env::var("UPDATE_FIXTURES").as_deref() == Ok("1") {
            fs::write(&path, bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        } else {
            let expected =
                fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert_eq!(bytes, expected, "fixture differs: {name}");
        }
    }
}
