//! WHY: Feature 00 (Layer 2) requires the WASM transport header to write/parse
//! exactly and `dispatch_message` to route each protocol byte to the right handler
//! (and panic on an unknown one). These integration tests exercise the public
//! transport API the way the runtime and the proc-macro wrappers will.
//!
//! WHAT: `WasmEnvelope` framing + the `dispatch_message` routing contract.
//!
//! HOW: Mock `ProtocolHandler`s record every `handle_from_js` call into a shared
//! log so we can assert exactly one handler fired with the right arguments.

use std::cell::RefCell;
use std::rc::Rc;

use foundation_wasm::{
    dispatch_message, MemoryId, ProtocolHandler, ProtocolHandlerRegistry, WasmEnvelope,
};

/// One entry per `handle_from_js` call: `(protocol, memory_id, payload_len)`.
type CallLog = Rc<RefCell<Vec<(u8, u64, usize)>>>;

/// Records every `handle_from_js` invocation into a shared log so the test can
/// assert which handler the dispatcher routed to.
struct RecordingHandler {
    protocol: u8,
    log: CallLog,
}

impl ProtocolHandler for RecordingHandler {
    fn protocol_byte(&self) -> u8 {
        self.protocol
    }
    fn version(&self) -> u8 {
        0
    }
    fn send_to_js(&self, _memory_id: MemoryId, _ptr: *const u8, _len: usize) {}
    fn handle_from_js(&self, memory_id: MemoryId, _ptr: *const u8, len: usize) {
        self.log
            .borrow_mut()
            .push((self.protocol, memory_id.as_u64(), len));
    }
}

/// Build a registry whose three handlers all write to one shared log.
fn registry_with_log() -> (ProtocolHandlerRegistry, CallLog) {
    let log: CallLog = Rc::new(RefCell::new(Vec::new()));
    let reg = ProtocolHandlerRegistry::new(
        Box::new(RecordingHandler {
            protocol: 0,
            log: log.clone(),
        }),
        Box::new(RecordingHandler {
            protocol: 1,
            log: log.clone(),
        }),
        Box::new(RecordingHandler {
            protocol: 2,
            log: log.clone(),
        }),
    );
    (reg, log)
}

#[test]
fn wasm_envelope_write_parse_round_trips() {
    let payload = b"arrow-ipc-bytes-here";
    let mem = MemoryId::new(3, 1).as_u64();
    let framed = WasmEnvelope::write(1, 0, mem, payload);
    assert_eq!(framed.len(), WasmEnvelope::HEADER_LEN + payload.len());
    let (env, body) = WasmEnvelope::parse(&framed);
    assert_eq!(env.protocol, 1);
    assert_eq!(env.version, 0);
    assert_eq!(env.memory_id, mem);
    assert_eq!(env.length as usize, payload.len());
    assert_eq!(body, payload);
}

#[test]
fn wasm_envelope_exact_byte_layout() {
    // memory_id 0x0000_0001_0000_0003 (index=1, generation=3)
    let mem = 0x0000_0001_0000_0003u64;
    let payload = [0xAAu8, 0xBB];
    let framed = WasmEnvelope::write(1, 0, mem, &payload);
    assert_eq!(framed[0], 1); // protocol
    assert_eq!(framed[1], 0); // version
    assert_eq!(&framed[2..10], &mem.to_le_bytes()); // memory_id LE
    assert_eq!(&framed[10..14], &2u32.to_le_bytes()); // length LE
    assert_eq!(&framed[14..16], &payload);
}

#[test]
fn try_parse_rejects_short_and_truncated() {
    assert!(WasmEnvelope::try_parse(&[0u8; 13]).is_none());
    let mut framed = WasmEnvelope::write(2, 0, 0, b"hi");
    framed[10] = 200; // claim 200-byte payload that isn't present
    assert!(WasmEnvelope::try_parse(&framed).is_none());
}

#[test]
fn dispatch_routes_each_protocol_to_its_handler() {
    for (proto, idx, gen) in [(0u8, 4u32, 1u32), (1, 7, 2), (2, 9, 3)] {
        let (reg, log) = registry_with_log();
        let mem = MemoryId::new(idx, gen).as_u64();
        let payload = b"payload-bytes";
        let framed = WasmEnvelope::write(proto, 0, mem, payload);
        dispatch_message(&framed, &reg);
        let calls = log.borrow();
        assert_eq!(calls.len(), 1, "exactly one handler should fire");
        assert_eq!(calls[0], (proto, mem, payload.len()));
    }
}

#[test]
#[should_panic(expected = "unknown protocol: 255")]
fn dispatch_panics_on_unknown_protocol() {
    let (reg, _log) = registry_with_log();
    let framed = WasmEnvelope::write(255, 0, 0, b"x");
    dispatch_message(&framed, &reg);
}
