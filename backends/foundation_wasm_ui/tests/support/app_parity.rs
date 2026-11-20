//! Compare both encoders at each flush of a real application scenario.
use super::parity::reference;
use foundation_ui_traits::{ColumnarEncoder, ProtocolEncoder};
use foundation_ui_wire::{OperationT, WireLimits};
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler};
use foundation_wasm_ui::{
    protocol::{decode_frame, WrittenBatch},
    App, FrameV1, HandleResult, MockProtocol, ProtocolMethods, SentBatches,
};

pub struct Compared {
    frame: FrameV1,
    mock: MockProtocol,
}

impl Compared {
    pub fn new() -> Self {
        Self {
            frame: FrameV1::new(7),
            mock: MockProtocol::new(),
        }
    }
    pub fn sent_batches(&self) -> SentBatches {
        self.mock.sent_batches()
    }
}

pub fn app() -> (App, SentBatches) {
    let protocol = Compared::new();
    let sent = protocol.sent_batches();
    (App::with_protocol(protocol), sent)
}

impl ProtocolHandler for Compared {
    fn protocol_byte(&self) -> u8 {
        self.mock.protocol_byte()
    }
    fn version(&self) -> u8 {
        self.mock.version()
    }
    fn send_to_js(&self, _: MemoryId, _: *const u8, _: usize) {}
    fn handle_from_js(&self, _: MemoryId, _: *const u8, _: usize) {}
    fn ack(&self, id: MemoryId, memory: &mut MemoryAllocations) {
        self.mock.ack(id, memory);
    }
}
impl ProtocolMethods<Vec<OperationT>> for Compared {
    fn encode_and_write(
        &self,
        ops: Vec<OperationT>,
        memory: &mut MemoryAllocations,
    ) -> WrittenBatch {
        let old: Vec<_> = ops.iter().map(reference).collect();
        let expected = ColumnarEncoder
            .decode(&ColumnarEncoder.encode(old))
            .unwrap();
        self.frame.deliver(ops.clone(), 0, |bytes| {
            let decoded = decode_frame(&bytes, &WireLimits::default())?;
            let effects: Vec<_> = decoded.iter().map(reference).collect();
            assert_eq!(
                effects, expected,
                "the two readers must produce the same ordered effects"
            );
            Ok(())
        })?;
        self.mock.encode_and_write(ops, memory)
    }
    fn handle_received(&self, bytes: &[u8]) -> HandleResult {
        self.mock.handle_received(bytes)
    }
}
