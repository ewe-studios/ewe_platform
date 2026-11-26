//! The record tree used by native conformance tests.
use super::{json, RefStore, Value};
use crate::{NodeKind, PropValueT};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Serialize)]
pub(super) struct Node {
    pub id: u32,
    pub kind: &'static str,
    pub text: Option<String>,
    pub props: BTreeMap<String, String>,
    pub children: Vec<u32>,
    pub actions: BTreeMap<String, String>,
    #[serde(skip)]
    pub state: Vec<u8>,
    pub state_len: usize,
    pub live: Option<Value>,
    pub web_view: Option<Value>,
    pub behaviours: BTreeSet<String>,
    #[serde(skip)]
    pub parent: Option<u32>,
}

impl Node {
    pub fn new(id: u32, kind: NodeKind) -> Self {
        Self {
            id,
            kind: kind.variant_name().unwrap_or("UNKNOWN"),
            text: None,
            props: BTreeMap::new(),
            children: Vec::new(),
            actions: BTreeMap::new(),
            state: Vec::new(),
            state_len: 0,
            live: None,
            web_view: None,
            behaviours: BTreeSet::new(),
            parent: None,
        }
    }
}

pub(super) fn prop_text(value: &PropValueT) -> Option<String> {
    use crate::LengthUnit;
    Some(match value {
        PropValueT::Length(v) => {
            let length = v.v.as_ref()?;
            match length.unit {
                LengthUnit::Point => format!("{}pt", length.value),
                LengthUnit::Percent => format!("{}%", length.value),
                LengthUnit::Auto => "auto".into(),
                _ => return None,
            }
        }
        PropValueT::FontToken(v) => format!("font:{}", v.id),
        PropValueT::ColorToken(v) => format!("color:{}", v.id),
        PropValueT::Alignment(v) => v.v.variant_name()?.into(),
        PropValueT::Axis(v) => v.v.variant_name()?.into(),
        PropValueT::Bool(v) => v.v.to_string(),
        PropValueT::Text(v) => v.v.clone().unwrap_or_default(),
        PropValueT::NONE => return None,
    })
}

impl RefStore {
    pub(super) fn node(&mut self, id: u32, row: usize) -> Option<&mut Node> {
        if !self.nodes.contains_key(&id) {
            self.reports
                .push(json!({"code":"UnknownNode","row":row,"node":id}));
        }
        self.nodes.get_mut(&id)
    }

    pub(super) fn detach(&mut self, id: u32) {
        let parent = self.nodes.get_mut(&id).and_then(|node| node.parent.take());
        if let Some(parent) = parent.and_then(|key| self.nodes.get_mut(&key)) {
            parent.children.retain(|child| *child != id);
        }
    }

    pub(super) fn remove(&mut self, id: u32) {
        self.detach(id);
        let mut pending = vec![id];
        while let Some(next) = pending.pop() {
            self.tags.remove(&next);
            if let Some(node) = self.nodes.remove(&next) {
                pending.extend(node.children);
            }
        }
    }

    pub(super) fn insert(&mut self, parent: u32, child: u32, before: Option<u32>, row: usize) {
        if self.node(parent, row).is_none() || self.node(child, row).is_none() {
            return;
        }
        if let Some(id) = before {
            if !self.nodes[&parent].children.contains(&id) {
                self.reports
                    .push(json!({"code":"UnknownNode","row":row,"node":id}));
                return;
            }
            if id == child {
                return;
            }
        }
        let mut ancestor = Some(parent);
        while let Some(id) = ancestor {
            if id == child || child == 1 {
                self.reports
                    .push(json!({"code":"InvalidParent","row":row,"node":child}));
                return;
            }
            ancestor = self.nodes[&id].parent;
        }
        self.detach(child);
        let children = &mut self
            .nodes
            .get_mut(&parent)
            .expect("checked parent")
            .children;
        let index = before
            .and_then(|id| children.iter().position(|child| *child == id))
            .unwrap_or(children.len());
        children.insert(index, child);
        self.nodes.get_mut(&child).expect("checked child").parent = Some(parent);
    }
}
