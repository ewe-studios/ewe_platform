use foundation_ui_wire::{
    finish_frame, ActionRequestT, FrameKindT, FrameRequestT, FrameT, SCHEMA_VERSION,
};

pub fn envelope(request: FrameRequestT, sequence: u64) -> Vec<u8> {
    let mut frame = FrameT::default();
    frame.schema_version = SCHEMA_VERSION;
    frame.stream_id = 7;
    frame.sequence_id = sequence;
    frame.kind = FrameKindT::FrameRequest(Box::new(request));
    finish_frame(&frame).unwrap()
}

pub fn request(reference: &str, node_id: u32, state: Option<Vec<u8>>, sequence: u64) -> Vec<u8> {
    let mut action = ActionRequestT::default();
    action.action = Some(reference.to_owned());
    action.node_id = node_id;
    action.state = state;
    let mut request = FrameRequestT::default();
    request.action = Some(Box::new(action));
    envelope(request, sequence)
}
