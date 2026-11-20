//! Build generated user-interface operations and message frames by hand.

use crate::{MessageT, OperationT, UiOperationT, WebOperationT};
use alloc::{boxed::Box, string::String};
use foundation_ui_traits::{AttrName, HtmlTag};

/// Wrap one operation as one frame message.
#[must_use]
pub fn message(operation: OperationT) -> MessageT {
    MessageT {
        operation: Some(Box::new(operation)),
        ..MessageT::default()
    }
}

fn shared(op: UiOperationT) -> OperationT {
    OperationT {
        ui: op,
        ..OperationT::default()
    }
}

fn web(op: WebOperationT) -> OperationT {
    OperationT {
        web: op,
        ..OperationT::default()
    }
}

#[must_use]
pub fn set_text(node_id: u32, text: &str) -> OperationT {
    shared(UiOperationT::SetText(Box::new(crate::SetTextT {
        node_id,
        text: Some(text.into()),
    })))
}

#[must_use]
pub fn append_child(parent_id: u32, child_id: u32) -> OperationT {
    shared(UiOperationT::AppendChild(Box::new(crate::AppendChildT {
        parent_id,
        child_id,
    })))
}

#[must_use]
pub fn remove_child(parent_id: u32, child_id: u32) -> OperationT {
    web(WebOperationT::WebDetachChild(Box::new(
        crate::WebDetachChildT {
            parent_id,
            child_id,
        },
    )))
}

#[must_use]
pub fn remove_node(node_id: u32) -> OperationT {
    shared(UiOperationT::Remove(Box::new(crate::RemoveT { node_id })))
}

#[must_use]
pub fn insert_before(parent_id: u32, child_id: u32, ref_id: u32) -> OperationT {
    shared(UiOperationT::InsertBefore(Box::new(crate::InsertBeforeT {
        parent_id,
        child_id,
        before_id: ref_id,
    })))
}

#[must_use]
pub fn create_text_node(node_id: u32, content: &str) -> OperationT {
    web(WebOperationT::WebText(Box::new(crate::WebTextT {
        node_id,
        content: Some(content.into()),
    })))
}

#[must_use]
pub fn replace_node(old_id: u32, new_id: u32) -> OperationT {
    web(WebOperationT::WebReplaceNode(Box::new(
        crate::WebReplaceNodeT { old_id, new_id },
    )))
}

#[must_use]
pub fn add_class(node_id: u32, class: &str) -> OperationT {
    web(WebOperationT::WebAddClass(Box::new(crate::WebAddClassT {
        node_id,
        class_name: Some(class.into()),
    })))
}

#[must_use]
pub fn remove_class(node_id: u32, class: &str) -> OperationT {
    web(WebOperationT::WebRemoveClass(Box::new(
        crate::WebRemoveClassT {
            node_id,
            class_name: Some(class.into()),
        },
    )))
}

#[must_use]
pub fn register_node(node_id: u32) -> OperationT {
    web(WebOperationT::WebRegisterNode(Box::new(
        crate::WebRegisterNodeT { node_id },
    )))
}

#[must_use]
pub fn unregister_node(node_id: u32) -> OperationT {
    web(WebOperationT::WebUnregisterNode(Box::new(
        crate::WebUnregisterNodeT { node_id },
    )))
}

#[must_use]
pub fn create_element(node_id: u32, tag: &HtmlTag, class: &str) -> OperationT {
    let (tag_id, custom_tag) = match tag {
        HtmlTag::Id(id) => (*id, None),
        HtmlTag::Name(name) => (0, Some(String::from(name.as_ref()))),
    };
    web(WebOperationT::WebElement(Box::new(crate::WebElementT {
        node_id,
        tag_id,
        custom_tag,
        class_name: Some(String::from(class)),
    })))
}

#[must_use]
pub fn set_attribute(node_id: u32, name: &AttrName, value: &str) -> OperationT {
    let (attr_id, custom_attr) = wire_attr(name);
    web(WebOperationT::WebSetAttribute(Box::new(
        crate::WebSetAttributeT {
            node_id,
            attr_id,
            custom_attr,
            value: Some(String::from(value)),
        },
    )))
}

#[must_use]
pub fn remove_attribute(node_id: u32, name: &AttrName) -> OperationT {
    let (attr_id, custom_attr) = wire_attr(name);
    web(WebOperationT::WebRemoveAttribute(Box::new(
        crate::WebRemoveAttributeT {
            node_id,
            attr_id,
            custom_attr,
        },
    )))
}

#[must_use]
pub fn set_property(node_id: u32, name: &AttrName, value: &str) -> OperationT {
    let (name_id, custom_name) = wire_attr(name);
    web(WebOperationT::WebProperty(Box::new(crate::WebPropertyT {
        node_id,
        name_id,
        custom_name,
        value: Some(String::from(value)),
    })))
}

#[must_use]
pub fn add_event_listener(node_id: u32, name: &AttrName) -> OperationT {
    let (name_id, custom_name) = wire_attr(name);
    web(WebOperationT::WebAddListener(Box::new(
        crate::WebAddListenerT {
            node_id,
            name_id,
            custom_name,
        },
    )))
}

#[must_use]
pub fn remove_event_listener(node_id: u32, name: &AttrName) -> OperationT {
    let (name_id, custom_name) = wire_attr(name);
    web(WebOperationT::WebRemoveListener(Box::new(
        crate::WebRemoveListenerT {
            node_id,
            name_id,
            custom_name,
        },
    )))
}

#[must_use]
pub fn set_style(node_id: u32, name: &AttrName, value: &str) -> OperationT {
    let (name_id, custom_name) = wire_attr(name);
    web(WebOperationT::WebStyle(Box::new(crate::WebStyleT {
        node_id,
        name_id,
        custom_name,
        value: Some(String::from(value)),
    })))
}

fn wire_attr(name: &AttrName) -> (u16, Option<String>) {
    match name {
        AttrName::Id(id) => (*id, None),
        AttrName::Name(name) => (0, Some(String::from(name.as_ref()))),
    }
}

/// A mounted node or a document selector for a markup change.
#[derive(Clone, Copy, Debug)]
pub enum MorphTarget<'a> {
    NodeId(u32),
    Id(&'a str),
    Class(&'a str),
    Query(&'a str),
}

#[must_use]
pub fn morph_node(
    target: MorphTarget<'_>,
    action: crate::MorphAction,
    content: &str,
) -> OperationT {
    use crate::SelectorKind;
    let (kind, node_id, text) = match target {
        MorphTarget::NodeId(id) => (SelectorKind::NodeId, id, None),
        MorphTarget::Id(s) => (SelectorKind::Id, 0, Some(s.into())),
        MorphTarget::Class(s) => (SelectorKind::Class, 0, Some(s.into())),
        MorphTarget::Query(s) => (SelectorKind::Query, 0, Some(s.into())),
    };
    web(WebOperationT::HtmlMorph(Box::new(crate::HtmlMorphT {
        target: Some(Box::new(crate::TargetSelectorT {
            kind,
            node_id,
            text,
        })),
        action,
        content: Some(content.into()),
    })))
}

/// Attach a shared or browser behaviour to the owning node.
#[must_use]
pub fn behaviour(node_id: u32, name: &str, mode: Option<&str>) -> OperationT {
    use crate::{BehaviourArgT, PropValueT, TextValueT, UiBehaviourT, WebBehaviourT};
    let args = mode.map(|mode| {
        alloc::vec![BehaviourArgT {
            name: Some("mode".into()),
            value: PropValueT::Text(Box::new(TextValueT {
                v: Some(mode.into())
            })),
        }]
    });
    match crate::vocab::shared_behaviour(name) {
        Some(behaviour) => shared(UiOperationT::UiBehaviour(Box::new(UiBehaviourT {
            node_id,
            behaviour,
            args,
        }))),
        None => web(WebOperationT::WebBehaviour(Box::new(WebBehaviourT {
            node_id,
            custom: Some(name.into()),
            args,
            ..Default::default()
        }))),
    }
}

/// Run an application script in the scope of its parent node.
#[must_use]
pub fn scoped_script(node_id: u32, parent_id: u32, body: String) -> OperationT {
    web(WebOperationT::ScriptNode(Box::new(crate::ScriptNodeT {
        node_id,
        parent_id,
        body: Some(body),
    })))
}
