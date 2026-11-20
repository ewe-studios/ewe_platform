use foundation_ui_traits::{AttrName, DomOp, HtmlTag, MorphAction, TargetSelector, ATTR_CLASS};
use foundation_ui_wire::{
    vocab::{dom_op, web_form, WireForm},
    UiOperationT, WebOperationT, WebRemoveAttributeT, WebSetAttributeT,
};

#[test]
fn each_dom_op_round_trips() {
    let id = 42;
    let name = AttrName::from_static("class");
    let custom_name = AttrName::from_static("data-cart");
    let cases = [
        DomOp::CreateTextNode {
            node_id: id,
            content: "text".into(),
        },
        DomOp::SetText {
            node_id: id,
            text: "1".into(),
        },
        DomOp::AppendChild {
            parent_id: 9,
            child_id: id,
        },
        DomOp::RemoveChild {
            parent_id: 9,
            child_id: id,
        },
        DomOp::RemoveNode { node_id: id },
        DomOp::InsertBefore {
            parent_id: 9,
            child_id: id,
            ref_id: 43,
        },
        DomOp::ReplaceNode {
            old_id: id,
            new_id: 43,
        },
        DomOp::AddClass {
            node_id: id,
            class: "btn".into(),
        },
        DomOp::RemoveClass {
            node_id: id,
            class: "btn".into(),
        },
        DomOp::RegisterNode { node_id: id },
        DomOp::UnregisterNode { node_id: id },
    ];
    for op in cases {
        assert_eq!(dom_op(&web_form(&op)), Some(op));
    }
    for attr in [name, custom_name] {
        for op in [
            DomOp::SetAttribute {
                node_id: id,
                name: attr.clone(),
                value: "btn".into(),
            },
            DomOp::RemoveAttribute {
                node_id: id,
                name: attr.clone(),
            },
            DomOp::SetProperty {
                node_id: id,
                name: attr.clone(),
                value: "true".into(),
            },
            DomOp::AddEventListener {
                node_id: id,
                event_name: attr.clone(),
            },
            DomOp::RemoveEventListener {
                node_id: id,
                event_name: attr.clone(),
            },
            DomOp::SetStyle {
                node_id: id,
                prop: attr,
                value: "red".into(),
            },
        ] {
            assert_eq!(dom_op(&web_form(&op)), Some(op));
        }
    }
    for tag in [
        HtmlTag::from_static("button"),
        HtmlTag::from_static("my-card"),
    ] {
        let op = DomOp::CreateElement {
            node_id: id,
            tag,
            class: "".into(),
        };
        assert_eq!(dom_op(&web_form(&op)), Some(op));
    }
}

#[test]
fn morph_selectors_and_actions_round_trip() {
    for target in [
        TargetSelector::NodeId(42),
        TargetSelector::Id("items".into()),
        TargetSelector::Class("item".into()),
        TargetSelector::Query("li:first-child".into()),
    ] {
        for action in [
            MorphAction::ReplaceChildren,
            MorphAction::ReplaceElement,
            MorphAction::InsertBefore,
            MorphAction::InsertAfter,
            MorphAction::AppendSibling,
        ] {
            let op = DomOp::MorphNode {
                target: target.clone(),
                action,
                content: "<li>1</li>".into(),
            };
            assert_eq!(dom_op(&web_form(&op)), Some(op));
        }
    }
}

#[test]
fn lowered_forms_keep_their_group_and_fields() {
    let node_id = 42;
    let name = AttrName::from_static("class");
    assert_eq!(
        web_form(&DomOp::SetAttribute {
            node_id,
            name: name.clone(),
            value: "btn".into()
        }),
        WireForm::Web(WebOperationT::WebSetAttribute(Box::new(table!(
            WebSetAttributeT {
                node_id: node_id,
                attr_id: ATTR_CLASS,
                value: Some("btn".into()),
                custom_attr: None,
            }
        )))),
    );
    assert_eq!(
        web_form(&DomOp::RemoveAttribute { node_id, name }),
        WireForm::Web(WebOperationT::WebRemoveAttribute(Box::new(table!(
            WebRemoveAttributeT {
                node_id: node_id,
                attr_id: ATTR_CLASS,
                custom_attr: None,
            }
        )))),
    );
    let WireForm::Web(WebOperationT::WebElement(element)) = web_form(&DomOp::CreateElement {
        node_id: 5,
        tag: HtmlTag::from_static("my-card"),
        class: "".into(),
    }) else {
        panic!("Expected a web element")
    };
    assert_eq!(element.node_id, 5);
    assert_eq!(element.tag_id, 0);
    assert_eq!(element.custom_tag.as_deref(), Some("my-card"));
    assert_eq!(element.class_name.as_deref(), Some(""));

    let WireForm::Ui(UiOperationT::SetText(text)) = web_form(&DomOp::SetText {
        node_id,
        text: "1".into(),
    }) else {
        panic!("Expected shared text")
    };
    assert_eq!(text.node_id, node_id);
    assert_eq!(text.text.as_deref(), Some("1"));
    assert_eq!(
        dom_op(&WireForm::Ui(UiOperationT::CreateNode(Box::default()))),
        None
    );
    assert_eq!(
        dom_op(&WireForm::Web(WebOperationT::WebBehaviour(Box::default()))),
        None
    );
}
