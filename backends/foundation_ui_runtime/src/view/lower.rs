use super::{
    properties::{core_kind, property},
    LowerError, LowerOptions,
};
use foundation_ui_traits::{AttrName, Html, HtmlTag};
use foundation_ui_wire::{
    vocab::{is_web_behaviour, shared_behaviour, WEB_EVENTS},
    AppendChildT, BindActionT, CreateNodeT, EventId, OperationT, ScriptNodeT, SetTextT,
    UiBehaviourT, UiOperationT, WebBehaviourT, WebElementT, WebOperationT, WebRegisterNodeT,
    WebSetAttributeT, WebTextT,
};
use std::collections::BTreeSet;

fn attr<'a>(html: &'a Html, name: &str) -> Option<&'a str> {
    html.attributes
        .iter()
        .find(|(key, _)| key.name() == Some(name))
        .map(|(_, v)| v.as_ref())
}

pub(super) fn ui(op: UiOperationT) -> OperationT {
    table!(OperationT { ui: op })
}

fn web(op: WebOperationT) -> OperationT {
    table!(OperationT { web: op })
}

pub(super) fn append(parent: u32, child: u32) -> OperationT {
    ui(UiOperationT::AppendChild(Box::new(table!(AppendChildT {
        parent_id: parent,
        child_id: child
    }))))
}

pub(super) fn attribute(id: u32, name: &AttrName, value: &str) -> OperationT {
    let (attr_id, custom_attr) = match name {
        AttrName::Id(id) => (*id, None),
        AttrName::Name(text) => (0, Some(text.to_string())),
    };
    web(WebOperationT::WebSetAttribute(Box::new(table!(
        WebSetAttributeT {
            node_id: id,
            attr_id: attr_id,
            custom_attr: custom_attr,
            value: Some(value.to_owned())
        }
    ))))
}

fn register(id: u32) -> OperationT {
    web(WebOperationT::WebRegisterNode(Box::new(table!(
        WebRegisterNodeT { node_id: id }
    ))))
}

pub(super) fn local_id(html: &Html) -> Result<u32, LowerError> {
    attr(html, "primal-id")
        .and_then(|n| n.parse().ok())
        .ok_or(LowerError::MissingId)
}

/// Count identifiers before writing any operation. Independently composed helpers can reuse a local identifier.
#[derive(Default)]
pub(super) struct Layout {
    known: BTreeSet<u32>,
    extra: u32,
}

impl Layout {
    pub(super) fn new(html: &Html) -> Result<Self, LowerError> {
        if html.tag.is_none() {
            return Err(LowerError::NoRootElement);
        }
        let mut layout = Self::default();
        layout.visit(html, false)?;
        Ok(layout)
    }

    fn visit(&mut self, html: &Html, web_parent: bool) -> Result<(), LowerError> {
        let mut web_children = web_parent;
        if let Some(tag) = &html.tag {
            if attr(html, "data-ewe-behaviour").is_some() {
                return Ok(());
            }
            if !self.known.insert(local_id(html)?) {
                self.extra = self.extra.checked_add(1).ok_or(LowerError::IdOverflow)?;
            }
            if attr(html, "primal:script").is_some() {
                return Ok(());
            }
            web_children = core_kind(tag.name().ok_or(LowerError::MissingId)?).is_none();
        } else if html.text.is_some() && web_parent {
            self.extra = self.extra.checked_add(1).ok_or(LowerError::IdOverflow)?;
        }
        for child in &html.children {
            self.visit(child, web_children)?;
        }
        Ok(())
    }

    pub(super) fn span(&self) -> Result<u32, LowerError> {
        let last = self
            .known
            .last()
            .copied()
            .ok_or(LowerError::NoRootElement)?;
        last.checked_add(self.extra)
            .and_then(|n| n.checked_add(1))
            .ok_or(LowerError::IdOverflow)
    }
}

pub(super) struct Tree {
    pub root: u32,
    pub ops: Vec<OperationT>,
    pub markers: Vec<u32>,
}

struct Builder<'a> {
    options: LowerOptions<'a>,
    next_local: u64,
    used: BTreeSet<u32>,
    ops: Vec<OperationT>,
    markers: Vec<u32>,
}

impl Builder<'_> {
    fn next_id(&mut self) -> Result<u32, LowerError> {
        let id = u64::from(self.options.id_base) + self.next_local;
        self.next_local += 1;
        u32::try_from(id).map_err(|_| LowerError::IdOverflow)
    }

    fn id(&mut self, n: u32, root: Option<u32>) -> Result<u32, LowerError> {
        if self.used.insert(n) {
            root.map_or_else(
                || {
                    self.options
                        .id_base
                        .checked_add(n)
                        .ok_or(LowerError::IdOverflow)
                },
                Ok,
            )
        } else {
            self.next_id()
        }
    }

    fn element(
        &mut self,
        html: &Html,
        parent: Option<u32>,
        root: Option<u32>,
    ) -> Result<Option<u32>, LowerError> {
        if let Some(name) = attr(html, "data-ewe-behaviour") {
            let parent = parent.ok_or(LowerError::NoRootElement)?;
            self.behaviour(parent, name)?;
            return Ok(None);
        }
        let tag = html.tag.as_ref().ok_or(LowerError::NoRootElement)?;
        let local = local_id(html)?;
        let id = self.id(local, root)?;
        if html.parts.iter().any(|part| part.marks_state_on(local)) {
            self.markers.push(id);
        }
        if html.attributes.iter().any(|(k, _)| {
            k.name()
                .is_some_and(|n| n.eq_ignore_ascii_case("primal::state"))
        }) {
            return Err(LowerError::ReservedAttribute);
        }
        let children = flattened(&html.children);
        if attr(html, "primal:script").is_some() {
            let script = table!(ScriptNodeT {
                node_id: id,
                parent_id: parent.unwrap_or(0),
                body: Some(text_content(&children))
            });
            self.ops
                .push(web(WebOperationT::ScriptNode(Box::new(script))));
            // ScriptNode attaches itself to its own parent.
            return Ok(parent.is_none().then_some(id));
        }
        let kind = core_kind(tag.name().ok_or(LowerError::MissingId)?);
        if let Some(kind) = kind {
            self.ops.push(ui(UiOperationT::CreateNode(Box::new(table!(
                CreateNodeT {
                    node_id: id,
                    kind: kind
                }
            )))));
        } else {
            let (tag_id, custom_tag) = match tag {
                HtmlTag::Id(number) => (*number, None),
                HtmlTag::Name(name) => (0, Some(name.to_string())),
            };
            let element = table!(WebElementT {
                node_id: id,
                tag_id: tag_id,
                custom_tag: custom_tag,
                class_name: Some(attr(html, "class").unwrap_or("").into())
            });
            self.ops
                .push(web(WebOperationT::WebElement(Box::new(element))));
            self.ops.push(register(id));
        }
        let core = kind.is_some();
        self.attributes(html, id, core)?;
        if core && children.iter().any(|n| n.text.is_some()) {
            self.ops
                .push(ui(UiOperationT::SetText(Box::new(table!(SetTextT {
                    node_id: id,
                    text: Some(text_content(&children))
                })))));
        }
        for child in children {
            let child_id = if child.tag.is_some() {
                self.element(child, Some(id), None)?
            } else if let Some(text) = child.text.as_deref().filter(|_| !core) {
                let tid = self.next_id()?;
                let node = table!(WebTextT {
                    node_id: tid,
                    content: Some(text.into())
                });
                self.ops.push(web(WebOperationT::WebText(Box::new(node))));
                self.ops.push(register(tid));
                Some(tid)
            } else {
                None
            };
            if let Some(child_id) = child_id {
                self.ops.push(append(id, child_id));
            }
        }
        Ok(Some(id))
    }

    fn attributes(&mut self, html: &Html, id: u32, core: bool) -> Result<(), LowerError> {
        for (name, value) in &html.attributes {
            let key = name.name().ok_or(LowerError::MissingId)?;
            let event = if let Some(event) = key.strip_prefix("ewe:on:") {
                Some(match event {
                    "tap" => EventId::TAP,
                    "change" => EventId::CHANGE,
                    "submit" => EventId::SUBMIT,
                    "focus" => EventId::FOCUS,
                    "blur" => EventId::BLUR,
                    _ => {
                        return Err(LowerError::BadProp {
                            node_id: id,
                            prop: key.into(),
                            value: value.to_string(),
                        })
                    }
                })
            } else if !core && (value.starts_with("host:") || value.starts_with("server:")) {
                key.strip_prefix("primal:on").and_then(|event| {
                    WEB_EVENTS
                        .iter()
                        .find(|(_, n)| *n == event)
                        .map(|(id, _)| *id)
                })
            } else {
                None
            };
            if let Some(event) = event {
                crate::action::ActionRef::parse(value).map_err(|_| LowerError::BadProp {
                    node_id: id,
                    prop: key.into(),
                    value: value.to_string(),
                })?;
                let binding = table!(BindActionT {
                    node_id: id,
                    event: event,
                    action: Some(value.to_string())
                });
                self.ops
                    .push(ui(UiOperationT::BindAction(Box::new(binding))));
            } else if core {
                if !matches!(key, "primal-id" | "key") {
                    self.ops.push(ui(UiOperationT::SetProp(Box::new(property(
                        id,
                        key,
                        value,
                        self.options.tokens,
                    )?))));
                }
            } else if key != "class" {
                self.ops.push(if key == "primal-id" {
                    attribute(id, name, &id.to_string())
                } else {
                    attribute(id, name, value)
                });
            }
        }
        Ok(())
    }

    fn behaviour(&mut self, id: u32, name: &str) -> Result<(), LowerError> {
        let op = if let Some(behaviour) = shared_behaviour(name) {
            ui(UiOperationT::UiBehaviour(Box::new(table!(UiBehaviourT {
                node_id: id,
                behaviour: behaviour
            }))))
        } else if is_web_behaviour(name) {
            web(WebOperationT::WebBehaviour(Box::new(table!(
                WebBehaviourT {
                    node_id: id,
                    custom: Some(name.into())
                }
            ))))
        } else {
            return Err(LowerError::BadProp {
                node_id: id,
                prop: "data-ewe-behaviour".into(),
                value: name.into(),
            });
        };
        self.ops.push(op);
        Ok(())
    }
}

fn flattened(children: &[Html]) -> Vec<&Html> {
    fn collect<'a>(children: &'a [Html], out: &mut Vec<&'a Html>) {
        for child in children {
            if child.tag.is_none() && child.text.is_none() {
                collect(&child.children, out);
            } else {
                out.push(child);
            }
        }
    }
    let mut out = Vec::new();
    collect(children, &mut out);
    out
}

fn text_content(children: &[&Html]) -> String {
    children
        .iter()
        .filter_map(|child| child.text.as_deref())
        .collect()
}

pub(super) fn build(
    html: &Html,
    options: &LowerOptions<'_>,
    layout: &Layout,
    root: Option<u32>,
) -> Result<Tree, LowerError> {
    let mut builder = Builder {
        next_local: u64::from(
            layout
                .known
                .last()
                .copied()
                .ok_or(LowerError::NoRootElement)?,
        ) + 1,
        options: *options,
        ops: Vec::new(),
        used: BTreeSet::new(),
        markers: Vec::new(),
    };
    let root = builder
        .element(html, None, root)?
        .ok_or(LowerError::NoRootElement)?;
    Ok(Tree {
        root,
        ops: builder.ops,
        markers: builder.markers,
    })
}

/// Lowers one element tree without attaching it to a parent.
///
/// # Errors
/// Reports invalid identifiers, bindings, properties, tokens or missing component state.
pub fn lower(html: &Html, options: &LowerOptions<'_>) -> Result<Vec<OperationT>, LowerError> {
    let tree = build(html, options, &Layout::new(html)?, None)?;
    if !tree.markers.is_empty() {
        return Err(LowerError::MissingState);
    }
    Ok(tree.ops)
}
