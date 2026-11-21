//! Read the shared placement trace through the generated native reader.
use foundation_ui_wire::{
    read_frame,
    writer::{write_messages, ClientCaps, FrameHeader, WriterConfig},
    CreateNodeT, MessageT, NodeKind, OperationT, PlaceT, PlacementKind, Platform, TargetSelectorT,
    UiOperationT, WireLimits,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Step {
    kind: String,
    target: u32,
    child: Option<u32>,
    #[serde(default)]
    root: u32,
    #[serde(default)]
    error: bool,
    order: Vec<u32>,
}

fn selector(id: u32) -> TargetSelectorT {
    let mut selector = TargetSelectorT::default();
    selector.node_id = id;
    selector
}

impl Step {
    fn message(&self) -> MessageT {
        let mut place = PlaceT::default();
        place.kind = *PlacementKind::ENUM_VALUES
            .iter()
            .find(|kind| kind.variant_name() == Some(self.kind.as_str()))
            .unwrap();
        place.target = Some(Box::new(selector(self.target)));
        place.child = self.child.map(selector).map(Box::new);
        place.root_node = self.root;
        if self.root != 0 {
            let mut node = CreateNodeT::default();
            node.node_id = self.root;
            node.kind = NodeKind::TEXT;
            let mut content = OperationT::default();
            content.ui = UiOperationT::CreateNode(Box::new(node));
            place.operations = Some(vec![content]);
        }
        let mut op = OperationT::default();
        op.ui = UiOperationT::Place(Box::new(place));
        let mut message = MessageT::default();
        message.operation = Some(Box::new(op));
        message
    }
}

// A native store with one ordered list at node 10. Node 21 belongs to another parent.
fn apply(order: &mut Vec<u32>, place: foundation_ui_wire::Place<'_>) -> Result<(), ()> {
    let target = place.target().unwrap().node_id();
    let index = |id| order.iter().position(|child| *child == id).ok_or(());
    match place.kind() {
        PlacementKind::Swap => {
            let i = index(target)?;
            order[i] = place.root_node();
        }
        PlacementKind::Remove => {
            let i = index(target)?;
            order.remove(i);
        }
        kind if target == 10 => {
            let child = place
                .child()
                .map(|child| index(child.node_id()))
                .transpose()?;
            let i = match kind {
                PlacementKind::Append => order.len(),
                PlacementKind::Before => child.unwrap_or(0),
                PlacementKind::After => child.map_or(order.len(), |i| i + 1),
                _ => return Err(()),
            };
            order.insert(i, place.root_node());
        }
        _ => return Err(()),
    }
    Ok(())
}

#[test]
fn native_reader_preserves_the_shared_placement_order() {
    let steps: Vec<Step> =
        serde_json::from_str(include_str!("../fixtures/placement_order.json")).unwrap();
    for platform in [Platform::Ios, Platform::Android] {
        let mut order = Vec::new();
        for step in &steps {
            let header = FrameHeader {
                stream_id: 8,
                sequence_id: 1,
                ack: 0,
            };
            let (bytes, report) = write_messages(
                vec![step.message()],
                &ClientCaps::native_current(platform),
                header,
                &WriterConfig::default(),
            )
            .unwrap();
            assert_eq!(report.filtered, Vec::new());
            let frame = read_frame(&bytes, &WireLimits::default()).unwrap();
            let message = frame
                .kind_as_frame_message()
                .unwrap()
                .messages()
                .unwrap()
                .get(0);
            let place = message.operation().unwrap().ui_as_place().unwrap();
            assert_eq!(apply(&mut order, place).is_err(), step.error);
            assert_eq!(order, step.order);
        }
    }
}
