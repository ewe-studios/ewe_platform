use alloc::{borrow::Cow, boxed::Box, string::String};

use foundation_ui_traits::{
    AttrName, DomOp, HtmlTag, MorphAction as DomMorphAction, TargetSelector as DomTargetSelector,
};

use super::WireForm;
use crate::{
    AppendChildT, HtmlMorphT, InsertBeforeT, MorphAction, RemoveT, SelectorKind, SetTextT,
    TargetSelectorT, UiOperationT, WebAddClassT, WebAddListenerT, WebElementT, WebOperationT,
    WebPropertyT, WebRegisterNodeT, WebRemoveAttributeT, WebRemoveClassT, WebRemoveListenerT,
    WebReplaceNodeT, WebSetAttributeT, WebStyleT, WebTextT, WebUnregisterNodeT,
};

/// Convert a legacy operation to its shared or web form.
#[must_use]
pub fn web_form(op: &DomOp) -> WireForm {
    match op {
        DomOp::SetText { node_id, text } => {
            WireForm::Ui(UiOperationT::SetText(Box::new(SetTextT {
                node_id: *node_id,
                text: Some(String::from(text.as_ref())),
            })))
        }
        DomOp::AppendChild {
            parent_id,
            child_id,
        } => WireForm::Ui(UiOperationT::AppendChild(Box::new(AppendChildT {
            parent_id: *parent_id,
            child_id: *child_id,
        }))),
        DomOp::RemoveChild {
            child_id: node_id, ..
        }
        | DomOp::RemoveNode { node_id } => WireForm::Ui(UiOperationT::Remove(Box::new(RemoveT {
            node_id: *node_id,
        }))),
        DomOp::InsertBefore {
            parent_id,
            child_id,
            ref_id,
        } => WireForm::Ui(UiOperationT::InsertBefore(Box::new(InsertBeforeT {
            parent_id: *parent_id,
            child_id: *child_id,
            before_id: *ref_id,
        }))),
        DomOp::CreateElement {
            node_id,
            tag,
            class,
        } => create_element(*node_id, tag, class),
        DomOp::CreateTextNode { node_id, content } => {
            WireForm::Web(WebOperationT::WebText(Box::new(WebTextT {
                node_id: *node_id,
                content: Some(String::from(content.as_ref())),
            })))
        }
        DomOp::SetAttribute {
            node_id,
            name,
            value,
        } => set_attribute(*node_id, name, value),
        DomOp::RemoveAttribute { node_id, name } => remove_attribute(*node_id, name),
        DomOp::SetProperty {
            node_id,
            name,
            value,
        } => set_property(*node_id, name, value),
        DomOp::AddEventListener {
            node_id,
            event_name,
        } => add_listener(*node_id, event_name),
        DomOp::RemoveEventListener {
            node_id,
            event_name,
        } => remove_listener(*node_id, event_name),
        DomOp::ReplaceNode { old_id, new_id } => {
            WireForm::Web(WebOperationT::WebReplaceNode(Box::new(WebReplaceNodeT {
                old_id: *old_id,
                new_id: *new_id,
            })))
        }
        DomOp::SetStyle {
            node_id,
            prop,
            value,
        } => set_style(*node_id, prop, value),
        DomOp::AddClass { node_id, class } => {
            WireForm::Web(WebOperationT::WebAddClass(Box::new(WebAddClassT {
                node_id: *node_id,
                class_name: Some(String::from(class.as_ref())),
            })))
        }
        DomOp::RemoveClass { node_id, class } => {
            WireForm::Web(WebOperationT::WebRemoveClass(Box::new(WebRemoveClassT {
                node_id: *node_id,
                class_name: Some(String::from(class.as_ref())),
            })))
        }
        DomOp::MorphNode {
            target,
            action,
            content,
        } => WireForm::Web(WebOperationT::HtmlMorph(Box::new(HtmlMorphT {
            target: Some(Box::new(wire_selector(target))),
            action: MorphAction(action.as_u8()),
            content: Some(String::from(content.as_ref())),
        }))),
        DomOp::RegisterNode { node_id } => {
            WireForm::Web(WebOperationT::WebRegisterNode(Box::new(WebRegisterNodeT {
                node_id: *node_id,
            })))
        }
        DomOp::UnregisterNode { node_id } => WireForm::Web(WebOperationT::WebUnregisterNode(
            Box::new(WebUnregisterNodeT { node_id: *node_id }),
        )),
    }
}

/// Convert a supported form to a legacy operation. A remove form removes the node.
#[must_use]
pub fn dom_op(form: &WireForm) -> Option<DomOp> {
    let op = match form {
        WireForm::Ui(op) => match op {
            UiOperationT::SetText(table) => DomOp::SetText {
                node_id: table.node_id,
                text: legacy_text(table.text.as_deref())?,
            },
            UiOperationT::AppendChild(table) => DomOp::AppendChild {
                parent_id: table.parent_id,
                child_id: table.child_id,
            },
            UiOperationT::Remove(table) => DomOp::RemoveNode {
                node_id: table.node_id,
            },
            UiOperationT::InsertBefore(table) => DomOp::InsertBefore {
                parent_id: table.parent_id,
                child_id: table.child_id,
                ref_id: table.before_id,
            },
            _ => return None,
        },
        WireForm::Web(op) => return web_dom_op(op),
    };
    Some(op)
}

fn web_dom_op(op: &WebOperationT) -> Option<DomOp> {
    let op = match op {
        WebOperationT::WebElement(table) => DomOp::CreateElement {
            node_id: table.node_id,
            tag: legacy_name(
                table.tag_id,
                table.custom_tag.as_deref(),
                HtmlTag::Id,
                HtmlTag::Name,
            ),
            class: legacy_text(table.class_name.as_deref()).unwrap_or_default(),
        },
        WebOperationT::WebText(table) => DomOp::CreateTextNode {
            node_id: table.node_id,
            content: legacy_text(table.content.as_deref())?,
        },
        WebOperationT::WebSetAttribute(table) => DomOp::SetAttribute {
            node_id: table.node_id,
            name: legacy_attr(table.attr_id, table.custom_attr.as_deref()),
            value: legacy_text(table.value.as_deref())?,
        },
        WebOperationT::WebRemoveAttribute(table) => DomOp::RemoveAttribute {
            node_id: table.node_id,
            name: legacy_attr(table.attr_id, table.custom_attr.as_deref()),
        },
        WebOperationT::WebProperty(table) => DomOp::SetProperty {
            node_id: table.node_id,
            name: legacy_attr(table.name_id, table.custom_name.as_deref()),
            value: legacy_text(table.value.as_deref())?,
        },
        WebOperationT::WebAddListener(table) => DomOp::AddEventListener {
            node_id: table.node_id,
            event_name: legacy_attr(table.name_id, table.custom_name.as_deref()),
        },
        WebOperationT::WebRemoveListener(table) => DomOp::RemoveEventListener {
            node_id: table.node_id,
            event_name: legacy_attr(table.name_id, table.custom_name.as_deref()),
        },
        WebOperationT::WebReplaceNode(table) => DomOp::ReplaceNode {
            old_id: table.old_id,
            new_id: table.new_id,
        },
        WebOperationT::WebStyle(table) => DomOp::SetStyle {
            node_id: table.node_id,
            prop: legacy_attr(table.name_id, table.custom_name.as_deref()),
            value: legacy_text(table.value.as_deref())?,
        },
        WebOperationT::WebAddClass(table) => DomOp::AddClass {
            node_id: table.node_id,
            class: legacy_text(table.class_name.as_deref())?,
        },
        WebOperationT::WebRemoveClass(table) => DomOp::RemoveClass {
            node_id: table.node_id,
            class: legacy_text(table.class_name.as_deref())?,
        },
        WebOperationT::HtmlMorph(table) => DomOp::MorphNode {
            target: legacy_selector(table.target.as_deref()?)?,
            action: DomMorphAction::from_u8(table.action.0)?,
            content: legacy_text(table.content.as_deref())?,
        },
        WebOperationT::WebRegisterNode(table) => DomOp::RegisterNode {
            node_id: table.node_id,
        },
        WebOperationT::WebUnregisterNode(table) => DomOp::UnregisterNode {
            node_id: table.node_id,
        },
        _ => return None,
    };
    Some(op)
}

fn create_element(node_id: u32, tag: &HtmlTag, class: &str) -> WireForm {
    let (tag_id, custom_tag) = match tag {
        HtmlTag::Id(id) => (*id, None),
        HtmlTag::Name(name) => (0, Some(String::from(name.as_ref()))),
    };
    WireForm::Web(WebOperationT::WebElement(Box::new(WebElementT {
        node_id,
        tag_id,
        custom_tag,
        class_name: Some(String::from(class)),
    })))
}

fn set_attribute(node_id: u32, name: &AttrName, value: &str) -> WireForm {
    let (attr_id, custom_attr) = wire_attr(name);
    WireForm::Web(WebOperationT::WebSetAttribute(Box::new(WebSetAttributeT {
        node_id,
        attr_id,
        custom_attr,
        value: Some(String::from(value)),
    })))
}

fn remove_attribute(node_id: u32, name: &AttrName) -> WireForm {
    let (attr_id, custom_attr) = wire_attr(name);
    WireForm::Web(WebOperationT::WebRemoveAttribute(Box::new(
        WebRemoveAttributeT {
            node_id,
            attr_id,
            custom_attr,
        },
    )))
}

fn set_property(node_id: u32, name: &AttrName, value: &str) -> WireForm {
    let (name_id, custom_name) = wire_attr(name);
    WireForm::Web(WebOperationT::WebProperty(Box::new(WebPropertyT {
        node_id,
        name_id,
        custom_name,
        value: Some(String::from(value)),
    })))
}

fn add_listener(node_id: u32, name: &AttrName) -> WireForm {
    let (name_id, custom_name) = wire_attr(name);
    WireForm::Web(WebOperationT::WebAddListener(Box::new(WebAddListenerT {
        node_id,
        name_id,
        custom_name,
    })))
}

fn remove_listener(node_id: u32, name: &AttrName) -> WireForm {
    let (name_id, custom_name) = wire_attr(name);
    WireForm::Web(WebOperationT::WebRemoveListener(Box::new(
        WebRemoveListenerT {
            node_id,
            name_id,
            custom_name,
        },
    )))
}

fn set_style(node_id: u32, name: &AttrName, value: &str) -> WireForm {
    let (name_id, custom_name) = wire_attr(name);
    WireForm::Web(WebOperationT::WebStyle(Box::new(WebStyleT {
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

fn legacy_attr(id: u16, custom: Option<&str>) -> AttrName {
    legacy_name(id, custom, AttrName::Id, AttrName::Name)
}

fn legacy_name<T>(
    id: u16,
    custom: Option<&str>,
    from_id: impl FnOnce(u16) -> T,
    from_name: impl FnOnce(Cow<'static, str>) -> T,
) -> T {
    if id == 0 {
        if let Some(name) = legacy_text(custom) {
            return from_name(name);
        }
    }
    from_id(id)
}

fn wire_selector(target: &DomTargetSelector) -> TargetSelectorT {
    let (kind, node_id, text) = match target {
        DomTargetSelector::NodeId(id) => (SelectorKind::NodeId, *id, None),
        DomTargetSelector::Id(text) => (SelectorKind::Id, 0, Some(String::from(text.as_ref()))),
        DomTargetSelector::Class(text) => {
            (SelectorKind::Class, 0, Some(String::from(text.as_ref())))
        }
        DomTargetSelector::Query(text) => {
            (SelectorKind::Query, 0, Some(String::from(text.as_ref())))
        }
    };
    TargetSelectorT {
        kind,
        node_id,
        text,
    }
}

fn legacy_selector(target: &TargetSelectorT) -> Option<DomTargetSelector> {
    Some(match target.kind {
        SelectorKind::NodeId => DomTargetSelector::NodeId(target.node_id),
        SelectorKind::Id => DomTargetSelector::Id(legacy_text(target.text.as_deref())?),
        SelectorKind::Class => DomTargetSelector::Class(legacy_text(target.text.as_deref())?),
        SelectorKind::Query => DomTargetSelector::Query(legacy_text(target.text.as_deref())?),
        _ => return None,
    })
}

fn legacy_text(text: Option<&str>) -> Option<Cow<'static, str>> {
    text.map(|value| Cow::Owned(String::from(value)))
}
