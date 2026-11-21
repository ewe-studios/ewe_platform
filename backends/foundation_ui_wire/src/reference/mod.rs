//! Reference node store for the native renderer conformance fixtures.
mod tree;
use crate::{
    read_frame, FrameMessageT, MessageT, MobileOperationT, NodeKind, OperationT, UiOperationT,
    WebOperationT, WireLimits,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tree::{prop_text, Node};

/// Applies the wire contract and produces a stable snapshot for other readers.
pub struct RefStore {
    nodes: BTreeMap<u32, Node>,
    reports: Vec<Value>,
    commands: Vec<Value>,
    errors: Vec<Value>,
    data: Vec<Value>,
    caps: Vec<Value>,
    cursors: BTreeMap<u64, u64>,
    pending: BTreeMap<(u64, u64), FrameMessageT>,
    limits: WireLimits,
}
impl Default for RefStore {
    fn default() -> Self {
        Self::new()
    }
}
impl RefStore {
    /// Create an empty screen with stack root 1.
    #[must_use]
    pub fn new() -> Self {
        Self::with_limits(WireLimits::default())
    }

    #[must_use]
    pub fn with_limits(limits: WireLimits) -> Self {
        Self {
            nodes: [(1, Node::new(1, NodeKind::STACK))].into(),
            reports: vec![],
            commands: vec![],
            errors: vec![],
            data: vec![],
            caps: vec![],
            cursors: BTreeMap::new(),
            pending: BTreeMap::new(),
            limits,
        }
    }

    /// Apply a verified message. A live sequence gap holds later messages until replay.
    #[must_use]
    pub fn receive(&mut self, bytes: &[u8]) -> bool {
        let Ok(frame) = read_frame(bytes, &self.limits) else {
            return false;
        };
        let Some(body) = frame.kind_as_frame_message() else {
            return false;
        };
        let (id, seq) = (frame.stream_id(), frame.sequence_id());
        if seq == 0 || body.ack() != 0 {
            self.apply(body.unpack());
            return true;
        }
        if let Some(last) = self.cursors.get(&id).copied() {
            if seq <= last {
                return true;
            }
            if seq - last > 1 {
                self.pending
                    .entry((id, seq))
                    .or_insert_with(|| body.unpack());
                return true;
            }
        }
        self.apply(body.unpack());
        let mut last = seq;
        while let Some((next, body)) = last
            .checked_add(1)
            .and_then(|n| self.pending.remove(&(id, n)).map(|b| (n, b)))
        {
            self.apply(body);
            last = next;
        }
        self.cursors.insert(id, last);
        true
    }

    /// Render the snapshot used in the Swift and Kotlin test suites.
    ///
    /// # Panics
    /// Panics if an internal snapshot field cannot serialize as JSON.
    #[must_use]
    pub fn snapshot_json(&self) -> String {
        #[derive(serde::Serialize)]
        struct Snapshot<'a> {
            nodes: Vec<&'a Node>,
            reports: &'a [Value],
            commands: &'a [Value],
            errors: &'a [Value],
            data: &'a [Value],
            #[serde(skip_serializing_if = "<[Value]>::is_empty")]
            caps: &'a [Value],
        }
        serde_json::to_string_pretty(&Snapshot {
            nodes: self.nodes.values().collect(),
            reports: &self.reports,
            commands: &self.commands,
            errors: &self.errors,
            data: &self.data,
            caps: &self.caps,
        })
        .expect("a native snapshot contains only JSON values")
    }

    fn apply(&mut self, body: FrameMessageT) {
        for (
            row,
            MessageT {
                operation,
                data,
                control,
                ..
            },
        ) in body.messages.unwrap_or_default().into_iter().enumerate()
        {
            if let Some(op) = operation {
                self.operation(*op, row);
            }
            if let Some(data) = data {
                let kind = if data.arrow.is_some() {
                    "ArrowIpc"
                } else if data.table.is_some() {
                    "Table"
                } else {
                    "MessagePack"
                };
                self.data
                    .push(json!({"data":kind,"target":data.target_node}));
            }
            if let Some(control) = control {
                if let Some(e) = control.error {
                    self.errors.push(
                        json!({"code":e.code.variant_name(),"message":e.message,"row":e.row_index}),
                    );
                }
                if control.state_expired.is_some() {
                    self.errors.push(json!({"code":"StateExpired"}));
                }
                if control.update_required.is_some() {
                    self.errors.push(json!({"code":"UpdateRequired"}));
                }
                if let Some(c) = control.caps {
                    self.caps.push(json!({"stream_id":c.stream_id,"resume_from":c.resume_from,"status":c.status.variant_name()}));
                }
            }
        }
    }

    fn operation(&mut self, op: OperationT, row: usize) {
        let empty = op.ui == UiOperationT::NONE
            && op.web == WebOperationT::NONE
            && op.mobile == MobileOperationT::NONE
            && op.columnar.is_none();
        if empty {
            self.reports.push(json!({"code":"UnknownOp","row":row}));
        }
        self.shared(op.ui, row);
        if op.web != WebOperationT::NONE {
            self.reports
                .push(json!({"code":"UnsupportedPlatformOp","row":row}));
        }
        match op.mobile {
            MobileOperationT::MountWebView(v) => {
                if let Some(n) = self.node(v.node_id, row) {
                    n.web_view = Some(json!({"parent_id":v.parent_id,"url":v.url}));
                }
            }
            MobileOperationT::NativeCommand(v) => {
                if let Some(name) = v.command.variant_name().filter(|s| *s != "UNKNOWN") {
                    let args: BTreeMap<_, _> = v
                        .args
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|arg| Some((arg.name?, prop_text(&arg.value)?)))
                        .collect();
                    self.commands.push(json!({"command":name,"args":args}));
                } else {
                    self.reports
                        .push(json!({"code":"UnknownCommand","row":row}));
                }
            }
            MobileOperationT::MobileBehaviour(v) => {
                self.behaviour(v.node_id, v.custom.as_deref().unwrap_or("UNKNOWN"), row);
            }
            MobileOperationT::NONE => {}
        }
        if let Some(batch) = op.columnar {
            match crate::writer::decode_batch(&batch, row) {
                Ok(rows) => {
                    for (index, op) in rows.into_iter().enumerate() {
                        self.shared(op, row + index);
                    }
                }
                Err(_) => self
                    .reports
                    .push(json!({"code":"InvalidColumnar","row":row})),
            }
        }
    }

    fn behaviour(&mut self, id: u32, name: &str, row: usize) {
        let known = crate::vocab::SHARED_BEHAVIOUR_NAMES
            .iter()
            .any(|(text, _)| text.eq_ignore_ascii_case(name));
        if self.node(id, row).is_none() {
            return;
        }
        if known {
            self.nodes
                .get_mut(&id)
                .expect("checked node")
                .behaviours
                .insert(name.to_uppercase());
        } else {
            self.reports
                .push(json!({"code":"UnknownBehaviour","row":row,"node":id,"name":name}));
        }
    }

    fn create(&mut self, id: u32, kind: NodeKind, row: usize) {
        if self.nodes.contains_key(&id) {
            self.remove(id);
            self.reports
                .push(json!({"code":"DuplicateNode","row":row,"node":id}));
        }
        if kind == NodeKind::UNKNOWN || kind.variant_name().is_none() {
            self.reports
                .push(json!({"code":"UnknownKind","row":row,"node":id}));
        }
        self.nodes.insert(id, Node::new(id, kind));
    }

    fn shared(&mut self, op: UiOperationT, row: usize) {
        use UiOperationT as U;
        match op {
            U::NONE => {}
            U::CreateNode(v) => self.create(v.node_id, v.kind, row),
            U::SetText(v) => {
                if let Some(n) = self.node(v.node_id, row) {
                    n.text = v.text;
                }
            }
            U::SetProp(v) => {
                if self.node(v.node_id, row).is_none() {
                    return;
                }
                if let Some((name, value)) = v
                    .prop
                    .variant_name()
                    .filter(|s| *s != "UNKNOWN")
                    .zip(prop_text(&v.value))
                {
                    self.nodes
                        .get_mut(&v.node_id)
                        .expect("checked node")
                        .props
                        .insert(name.into(), value);
                } else {
                    self.reports
                        .push(json!({"code":"UnknownProp","row":row,"node":v.node_id}));
                }
            }
            U::AppendChild(v) => self.insert(v.parent_id, v.child_id, None, row),
            U::InsertBefore(v) => self.insert(v.parent_id, v.child_id, Some(v.before_id), row),
            U::Remove(v) => {
                if self.node(v.node_id, row).is_some() {
                    self.remove(v.node_id);
                }
            }
            U::ReplaceSubtree(v) => {
                if let Some(n) = self.node(v.node_id, row) {
                    for child in std::mem::take(&mut n.children) {
                        self.remove(child);
                    }
                    for op in v.operations.unwrap_or_default() {
                        self.operation(op, row);
                    }
                }
            }
            U::BindAction(v) => {
                if let Some(n) = self.node(v.node_id, row) {
                    if let (Some(name), Some(action)) = (v.event.variant_name(), v.action) {
                        n.actions.insert(name.into(), action);
                    }
                }
            }
            U::UnbindAction(v) => {
                if let Some(n) = self.node(v.node_id, row) {
                    if let Some(name) = v.event.variant_name() {
                        n.actions.remove(name);
                    }
                }
            }
            U::AttachState(v) => {
                if let Some(n) = self.node(v.root_node, row) {
                    n.state = v.state.unwrap_or_default();
                    n.state_len = n.state.len();
                }
            }
            U::LiveMount(v) => {
                self.create(v.node_id, NodeKind::STACK, row);
                self.nodes.get_mut(&v.node_id).expect("created node").live = Some(
                    json!({"kind":v.kind.variant_name(),"channel_id":v.channel_id,"transport":v.transport.variant_name(),"endpoint":v.endpoint,"resume_from":v.resume_from}),
                );
            }
            U::UiBehaviour(v) => self.behaviour(
                v.node_id,
                v.behaviour.variant_name().unwrap_or("UNKNOWN"),
                row,
            ),
            U::Place(v) => self.place(*v, row),
        }
    }

    fn place(&mut self, place: crate::PlaceT, row: usize) {
        use crate::{PlacementKind as K, SelectorKind};
        if place.kind == K::FullView {
            for id in self.nodes[&1].children.clone() {
                self.remove(id);
            }
            for op in place.operations.unwrap_or_default() {
                self.operation(op, row);
            }
            self.insert(1, place.root_node, None, row);
            return;
        }
        let Some(target) = place.target else { return };
        if target.kind != SelectorKind::NodeId
            || place
                .child
                .as_ref()
                .is_some_and(|child| child.kind != SelectorKind::NodeId)
        {
            self.reports
                .push(json!({"code":"UnsupportedSelector","row":row}));
            return;
        }
        let id = target.node_id;
        if self.node(id, row).is_none() {
            return;
        }
        if place.kind == K::Remove {
            self.remove(id);
            return;
        }
        let parent = if place.kind == K::Swap {
            self.nodes[&id].parent
        } else {
            Some(id)
        };
        let Some(parent) = parent else {
            self.reports
                .push(json!({"code":"InvalidParent","row":row,"node":id}));
            return;
        };
        let children = &self.nodes[&parent].children;
        let anchor = place.child.map(|s| s.node_id);
        if let Some(missing) = anchor.filter(|n| !children.contains(n)) {
            self.reports
                .push(json!({"code":"UnknownNode","row":row,"node":missing}));
            return;
        }
        let next = |id| {
            children
                .iter()
                .position(|n| *n == id)
                .and_then(|i| children.get(i + 1))
                .copied()
        };
        let before = match place.kind {
            K::Swap => next(id),
            K::Before => anchor.or_else(|| children.first().copied()),
            K::After => anchor.and_then(next),
            K::Append => None,
            _ => return,
        };
        if place.kind == K::Swap {
            self.remove(id);
        }
        for op in place.operations.unwrap_or_default() {
            self.operation(op, row);
        }
        self.insert(parent, place.root_node, before, row);
    }
}
