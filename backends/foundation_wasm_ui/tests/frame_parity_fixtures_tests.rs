//! Compare browser instruction encoders for every legacy operation.
use foundation_ui_traits::{
    encode_with_envelope, AttrName, ColumnarEncoder, DomOp, HtmlTag, MorphAction, TargetSelector,
};
use foundation_ui_wire::{
    ops, vocab::web_form,
    writer::{write_messages, ClientCaps, FrameHeader, WriterConfig},
    MessageT, OperationT,
};
use std::{fs, path::Path};

fn element(id: u32) -> DomOp {
    DomOp::CreateElement {
        node_id: id,
        tag: HtmlTag::from_static("button"),
        class: "button".into(),
    }
}

fn fixture(name: &str, bytes: &[u8]) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("integration/fixtures/frames");
    let path = dir.join(name);
    if std::env::var_os("UPDATE_FRAME_FIXTURES").is_some() {
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, bytes).unwrap();
    }
    assert_eq!(fs::read(&path).unwrap(), bytes, "Fixture {name} differs");
}

#[test]
fn every_dom_op_has_equivalent_frames() {
    use DomOp::*;
    let node_id = 10;
    let name = AttrName::from_static("title");
    let event_name = AttrName::from_static("click");
    let set = SetAttribute {
        node_id,
        name: name.clone(),
        value: "label".into(),
    };
    let add = AddEventListener {
        node_id,
        event_name: event_name.clone(),
    };
    let class = AddClass {
        node_id,
        class: "active".into(),
    };
    let attach = AppendChild {
        parent_id: 10,
        child_id: 11,
    };
    let cases = [
        ("create_element", vec![], element(12)),
        (
            "create_text",
            vec![],
            CreateTextNode {
                node_id: 12,
                content: "body".into(),
            },
        ),
        (
            "set_text",
            vec![],
            SetText {
                node_id,
                text: "next".into(),
            },
        ),
        ("set_attribute", vec![], set.clone()),
        (
            "remove_attribute",
            vec![set],
            RemoveAttribute { node_id, name },
        ),
        (
            "set_property",
            vec![],
            SetProperty {
                node_id,
                name: AttrName::from_static("checked"),
                value: "true".into(),
            },
        ),
        ("add_listener", vec![], add.clone()),
        (
            "remove_listener",
            vec![add],
            RemoveEventListener {
                node_id,
                event_name,
            },
        ),
        ("append_child", vec![], attach.clone()),
        (
            "remove_child",
            vec![attach.clone()],
            RemoveChild {
                parent_id: 10,
                child_id: 11,
            },
        ),
        (
            "remove_node",
            vec![attach.clone()],
            RemoveNode { node_id: 11 },
        ),
        (
            "insert_before",
            vec![attach.clone(), element(12), RegisterNode { node_id: 12 }],
            InsertBefore {
                parent_id: 10,
                child_id: 12,
                ref_id: 11,
            },
        ),
        (
            "replace_node",
            vec![attach, element(12)],
            ReplaceNode {
                old_id: 11,
                new_id: 12,
            },
        ),
        (
            "set_style",
            vec![],
            SetStyle {
                node_id,
                prop: AttrName::from_static("color"),
                value: "red".into(),
            },
        ),
        ("add_class", vec![], class.clone()),
        (
            "remove_class",
            vec![class],
            RemoveClass {
                node_id,
                class: "active".into(),
            },
        ),
        (
            "morph",
            vec![],
            MorphNode {
                target: TargetSelector::NodeId(node_id),
                action: MorphAction::ReplaceChildren,
                content: "new text".into(),
            },
        ),
        ("register", vec![element(12)], RegisterNode { node_id: 12 }),
        ("unregister", vec![], UnregisterNode { node_id }),
    ];
    let title = AttrName::from_static("title");
    let click = AttrName::from_static("click");
    let expected_operations = [
        ops::create_element(12,&HtmlTag::from_static("button"),"button"),
        ops::create_text_node(12,"body"), ops::set_text(10,"next"),
        ops::set_attribute(10,&title,"label"),ops::remove_attribute(10,&title),
        ops::set_property(10,&AttrName::from_static("checked"),"true"),
        ops::add_event_listener(10,&click),ops::remove_event_listener(10,&click),
        ops::append_child(10,11),ops::remove_child(10,11),ops::remove_node(11),
        ops::insert_before(10,12,11),ops::replace_node(11,12),
        ops::set_style(10,&AttrName::from_static("color"),"red"),
        ops::add_class(10,"active"),ops::remove_class(10,"active"),
        ops::morph_node(ops::MorphTarget::NodeId(10),foundation_ui_wire::MorphAction::ReplaceChildren,"new text"),
        ops::register_node(12),ops::unregister_node(10)
    ];
    assert_eq!(cases.len(),expected_operations.len());
    for ((name, setup, op), generated) in cases.into_iter().zip(expected_operations) {
        let mut ops = vec![element(10),RegisterNode {node_id:10},element(11),RegisterNode {node_id:11}];
        ops.extend(setup);
        let mut messages:Vec<_> = ops.iter().map(|op|ops::message(web_form(op).into())).collect();
        messages.push(ops::message(generated));
        ops.push(op);
        let (bytes, report) = write_messages(
            messages,
            &ClientCaps::web_current(),
            FrameHeader {
                stream_id: 1,
                sequence_id: 0,
                ack: 0,
            },
            &WriterConfig::default(),
        )
        .unwrap();
        assert!(report.filtered.is_empty());
        fixture(
            &format!("{name}.old.bin"),
            &encode_with_envelope(&ColumnarEncoder, ops),
        );
        fixture(&format!("{name}.new.bin"), &bytes);
    }
}

#[test]
fn shared_operations_have_row_and_column_fixtures() {
    use foundation_ui_wire::{read_frame, writer::columnar_batch, UiOperationT, WireLimits};

    let sample = include_bytes!("../../foundation_ui_wire/fixtures/frame_ops_all_forms.bin");
    let messages = read_frame(sample, &WireLimits::default())
        .unwrap()
        .kind_as_frame_message()
        .unwrap()
        .messages()
        .unwrap();
    let mut rows = Vec::new();
    for input in messages {
        let mut row = input.operation().unwrap().unpack().ui;
        match &mut row {
            UiOperationT::CreateNode(_)
            | UiOperationT::SetText(_)
            | UiOperationT::SetProp(_)
            | UiOperationT::AppendChild(_)
            | UiOperationT::Remove(_)
            | UiOperationT::BindAction(_)
            | UiOperationT::UnbindAction(_)
            | UiOperationT::AttachState(_) => (),
            UiOperationT::InsertBefore(value) => {
                value.child_id = 3;
                value.before_id = 2;
            }
            _ => continue,
        }
        rows.push(row);
    }
    assert_eq!(rows.len(), 9);
    let mut columns = OperationT::default();
    columns.columnar = Some(Box::new(columnar_batch(&rows).unwrap()));
    let units = rows
        .into_iter()
        .map(|ui| {
            let mut op = OperationT::default();
            op.ui = ui;
            op
        })
        .collect::<Vec<_>>();
    for (name, ops) in [
        ("shared-units.bin", units),
        ("shared-columns.bin", vec![columns]),
    ] {
        let messages = ops
            .into_iter()
            .map(|op| {
                let mut message = MessageT::default();
                message.operation = Some(Box::new(op));
                message
            })
            .collect();
        let (bytes, report) = write_messages(
            messages,
            &ClientCaps::web_current(),
            FrameHeader {
                stream_id: 1,
                sequence_id: 0,
                ack: 0,
            },
            &WriterConfig::default(),
        )
        .unwrap();
        assert!(report.filtered.is_empty());
        fixture(name, &bytes);
    }
}
