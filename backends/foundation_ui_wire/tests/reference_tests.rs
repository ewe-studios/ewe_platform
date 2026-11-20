//! Encode the same inputs used by the native renderer packages.
use foundation_ui_wire::{
    reference::RefStore,
    writer::{columnar_batch, write_messages, ClientCaps, FrameHeader, WriterConfig},
    *,
};
use std::{fs, path::Path};

fn shared(ui: UiOperationT) -> OperationT {
    table!(OperationT { ui: ui })
}
fn create(id: u32, kind: NodeKind) -> OperationT {
    shared(UiOperationT::CreateNode(Box::new(table!(CreateNodeT {
        node_id: id,
        kind: kind
    }))))
}
fn tree() -> Vec<OperationT> {
    vec![
        create(40, NodeKind::STACK),
        shared(UiOperationT::SetProp(Box::new(table!(SetPropT {
            node_id: 40,
            prop: PropId::PADDING,
            value: PropValueT::Length(Box::new(table!(LengthValueT {
                v: Some(table!(LengthT {
                    value: 8.0,
                    unit: LengthUnit::Point
                }))
            })))
        })))),
        shared(UiOperationT::SetProp(Box::new(table!(SetPropT {
            node_id: 40,
            prop: PropId::AXIS,
            value: PropValueT::Axis(Box::new(table!(AxisValueT { v: Axis::Vertical })))
        })))),
        create(41, NodeKind::TEXT),
        ops::set_text(41, "Count"),
        ops::append_child(40, 41),
        create(42, NodeKind::BUTTON),
        ops::set_text(42, "0"),
        shared(UiOperationT::BindAction(Box::new(table!(BindActionT {
            node_id: 42,
            event: EventId::TAP,
            action: Some("server:increment_counter".into())
        })))),
        ops::append_child(40, 42),
        ops::append_child(1, 40),
        shared(UiOperationT::AttachState(Box::new(table!(AttachStateT {
            root_node: 42,
            state: Some(include_bytes!("../fixtures/state_counter.bin").to_vec())
        })))),
    ]
}
fn mixed() -> Vec<OperationT> {
    vec![
        ops::set_text(42, "a"),
        table!(OperationT {
            web: WebOperationT::HtmlMorph(Box::new(table!(HtmlMorphT {
                target: Some(Box::new(table!(TargetSelectorT { node_id: 40 }))),
                action: MorphAction::ReplaceChildren,
                content: Some("<b>x</b>".into())
            })))
        }),
        table!(OperationT {
            mobile: MobileOperationT::NativeCommand(Box::new(table!(NativeCommandT {
                command: NativeCommandId::HAPTIC
            })))
        }),
        ops::set_text(999, "x"),
        ops::set_text(42, "b"),
    ]
}
fn encode(operations: Vec<OperationT>, sequence: u64, unfiltered: bool) -> Vec<u8> {
    let messages = operations.into_iter().map(ops::message).collect::<Vec<_>>();
    if unfiltered {
        return finish_frame(&table!(FrameT {
            schema_version: SCHEMA_VERSION,
            stream_id: 7,
            sequence_id: sequence,
            kind: FrameKindT::FrameMessage(Box::new(table!(FrameMessageT {
                ack: sequence,
                messages: Some(messages)
            })))
        }))
        .unwrap();
    }
    write_messages(
        messages,
        &ClientCaps::native_current(Platform::Ios),
        FrameHeader {
            stream_id: 7,
            sequence_id: sequence,
            ack: sequence,
        },
        &WriterConfig::default(),
    )
    .unwrap()
    .0
}
fn fixture(file: &str, bytes: &[u8]) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/native");
    let path = dir.join(file);
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, bytes).unwrap();
    }
    assert_eq!(fs::read(&path).unwrap(), bytes, "{file} differs");
}
#[test]
fn native_packages_share_reference_snapshots() {
    let base = encode(tree(), 1, false);
    let rows: Vec<_> = tree().into_iter().map(|op| op.ui).collect();
    let columns = table!(OperationT {
        columnar: Some(Box::new(columnar_batch(&rows).unwrap()))
    });
    let replacement = shared(UiOperationT::ReplaceSubtree(Box::new(table!(
        ReplaceSubtreeT {
            node_id: 40,
            operations: Some(vec![
                create(43, NodeKind::TEXT),
                ops::set_text(43, "new"),
                ops::append_child(40, 43)
            ])
        }
    ))));
    let behaviour = shared(UiOperationT::UiBehaviour(Box::new(table!(UiBehaviourT {
        node_id: 40,
        behaviour: BehaviourId::SCROLL_LOCK
    }))));
    let mut typed_snapshot = None;
    for (name, bytes, needs_tree) in [
        ("native_tree", base.clone(), false),
        ("native_columnar", encode(vec![columns], 1, false), false),
        ("native_mixed", encode(mixed(), 2, true), true),
        ("native_replace", encode(vec![replacement], 3, false), true),
        (
            "native_unknown_kind",
            encode(
                vec![create(60, NodeKind::UNKNOWN), ops::append_child(1, 60)],
                4,
                false,
            ),
            false,
        ),
        ("native_behaviour", encode(vec![behaviour], 5, false), true),
    ] {
        let mut store = RefStore::new();
        if needs_tree {
            assert!(store.receive(&base));
        }
        assert!(store.receive(&bytes));
        let snapshot = store.snapshot_json() + "\n";
        if name == "native_tree" {
            typed_snapshot = Some(snapshot.clone());
        }
        if name == "native_columnar" {
            assert_eq!(Some(&snapshot), typed_snapshot.as_ref());
        }
        fixture(&format!("{name}.bin"), &bytes);
        fixture(&format!("{name}.expected.json"), snapshot.as_bytes());
    }
}

#[test]
fn a_rejected_buffer_preserves_the_tree() {
    let good = include_bytes!("../fixtures/native/native_tree.bin");
    let mut store = RefStore::new();
    assert!(store.receive(good));
    let snapshot = store.snapshot_json();
    for bytes in [&good[..4], &good[..12], b"unknown".as_slice()] {
        assert!(!store.receive(bytes));
        assert_eq!(store.snapshot_json(), snapshot);
    }
}

#[test]
fn mixed_rows_report_errors_and_apply_the_remaining_rows() {
    let mut store = RefStore::new();
    assert!(store.receive(include_bytes!("../fixtures/native/native_tree.bin")));
    assert!(store.receive(include_bytes!("../fixtures/native/native_mixed.bin")));
    let result: serde_json::Value = serde_json::from_str(&store.snapshot_json()).unwrap();
    assert_eq!(result["nodes"][3]["text"], "b");
    assert_eq!(
        result["nodes"][3]["state_len"],
        include_bytes!("../fixtures/state_counter.bin").len()
    );
    assert_eq!(
        result["reports"][0],
        serde_json::json!({"code":"UnsupportedPlatformOp","row":1})
    );
    assert_eq!(
        result["reports"][1],
        serde_json::json!({"code":"UnknownNode","row":3,"node":999})
    );
    assert_eq!(result["commands"][0]["command"], "HAPTIC");
}

#[test]
fn ordered_pushes_hold_gaps_and_ignore_duplicates() {
    let mut store = RefStore::new();
    let frame = |seq: u64, text: &str| {
        write_messages(
            vec![ops::message(ops::set_text(1, text))],
            &ClientCaps::native_current(Platform::Android),
            FrameHeader {
                stream_id: 7,
                sequence_id: seq,
                ack: 0,
            },
            &WriterConfig::default(),
        )
        .unwrap()
        .0
    };
    let displayed = |s: &RefStore| {
        serde_json::from_str::<serde_json::Value>(&s.snapshot_json()).unwrap()["nodes"][0]["text"]
            .clone()
    };
    assert!(store.receive(&frame(1, "first")));
    assert!(store.receive(&frame(3, "third")));
    assert_eq!(displayed(&store), "first");
    assert!(store.receive(&frame(2, "second")));
    assert_eq!(displayed(&store), "third");
    assert!(store.receive(&frame(2, "duplicate")));
    assert_eq!(displayed(&store), "third");
}
