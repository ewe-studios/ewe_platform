//! A custom protocol can delegate storage and delivery to an existing codec.
use foundation_ui_wire::{ops, OperationT, UiOperationT};
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler};
use foundation_wasm_ui::{protocol::WrittenBatch, FrameV1, HandleResult, ProtocolMethods};

// Delegate the host and allocation lifecycle to the frame protocol.
struct Transport<P>(P);

impl<P: ProtocolHandler> ProtocolHandler for Transport<P> {
    fn protocol_byte(&self) -> u8 {
        self.0.protocol_byte()
    }
    fn version(&self) -> u8 {
        self.0.version()
    }
    fn send_to_js(&self, id: MemoryId, ptr: *const u8, len: usize) {
        self.0.send_to_js(id, ptr, len);
    }
    fn handle_from_js(&self, id: MemoryId, ptr: *const u8, len: usize) {
        self.0.handle_from_js(id, ptr, len);
    }
    fn ack(&self, id: MemoryId, memory: &mut MemoryAllocations) {
        self.0.ack(id, memory);
    }
}

impl ProtocolMethods<Vec<OperationT>> for Transport<FrameV1> {
    fn encode_and_write(
        &self,
        ops: Vec<OperationT>,
        memory: &mut MemoryAllocations,
    ) -> WrittenBatch {
        self.0.encode_and_write(ops, memory)
    }
    fn handle_received(&self, frame: &[u8]) -> HandleResult {
        self.0.handle_received(frame)
    }
}

fn main() -> Result<(), foundation_wasm_ui::ProtocolError> {
    let mut memory = MemoryAllocations::new();
    let new = Transport(FrameV1::new(1));
    let (slot, _, _) = new.encode_and_write(vec![ops::set_text(7, "ready")], &mut memory)?;
    let allocation = memory.get(slot.memory_id)?;
    let operations = allocation
        .into_with(|frame| new.handle_received(frame))
        .unwrap()?;
    assert!(matches!(&operations[0].ui, UiOperationT::SetText(text) if text.node_id == 7));
    new.ack(slot.memory_id, &mut memory);
    Ok(())
}
