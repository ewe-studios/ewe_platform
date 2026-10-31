//! Resumable `WebSocketFrameDecoder` tests (moved from `src/websocket/shared/decoder.rs`).

use std::io::Cursor;

use foundation_core::io::{read_frame_blocking, DecodeStep, IncrementalDecoder};
use foundation_netio::websocket::{Opcode, WebSocketFrameDecoder};

/// A frame split across 1-byte reads — the canonical incremental test.
#[test]
fn frame_split_across_many_reads_decodes() {
    let wire: Vec<u8> = vec![0x81, 0x05, b'H', b'e', b'l', b'l', b'o'];
    let mut dec = WebSocketFrameDecoder::new();

    for (i, &b) in wire.iter().enumerate() {
        let buf = [b];
        let mut cursor = Cursor::new(&buf);
        let result = dec.step(&mut cursor).expect("step");
        if i == wire.len() - 1 {
            match result {
                DecodeStep::Frame(frame) => {
                    assert!(frame.fin);
                    assert_eq!(frame.opcode, Opcode::Text);
                    assert_eq!(frame.payload, b"Hello".to_vec());
                    assert!(frame.mask.is_none());
                }
                DecodeStep::Pending => panic!("expected Frame on last byte"),
            }
        } else {
            assert_eq!(result, DecodeStep::Pending, "byte {i} = {b:#04x}");
        }
    }
}

/// Two complete frames back-to-back in a single read.
#[test]
fn two_frames_in_one_read() {
    let wire: Vec<u8> = vec![0x81, 0x02, b'H', b'i', 0x81, 0x02, b'Y', b'o'];
    let cursor = Cursor::new(wire);
    let mut dec = WebSocketFrameDecoder::new();

    let f1 = match dec.step(&mut cursor.clone()).expect("step") {
        DecodeStep::Frame(f) => f,
        other => panic!("expected Frame, got {other:?}"),
    };
    assert_eq!(f1.payload, b"Hi");

    // After consuming f1, the remaining bytes are still in our cursor.
    // Actually step() advances the cursor. Let's re-approach.
    // The cursor must be the same across steps.
    // Let's just pass all bytes at once and step twice.
    let wire: Vec<u8> = vec![0x81, 0x02, b'H', b'i', 0x81, 0x02, b'Y', b'o'];
    let mut cursor = Cursor::new(&wire);
    let mut dec = WebSocketFrameDecoder::new();

    let f1 = match dec.step(&mut cursor).expect("step") {
        DecodeStep::Frame(f) => f,
        other => panic!("expected Frame, got {other:?}"),
    };
    assert_eq!(f1.payload, b"Hi");

    let f2 = match dec.step(&mut cursor).expect("step") {
        DecodeStep::Frame(f) => f,
        other => panic!("expected Frame, got {other:?}"),
    };
    assert_eq!(f2.payload, b"Yo");
}

/// Masked frame (client→server, per RFC 6455).
#[test]
fn masked_frame_decodes() {
    let mask: [u8; 4] = [0x01, 0x02, 0x03, 0x04];
    let payload: &[u8] = b"Hello";
    let mut wire = vec![0x81, 0x85];
    wire.extend_from_slice(&mask);
    for (i, &b) in payload.iter().enumerate() {
        wire.push(b ^ mask[i % 4]);
    }

    let mut cursor = Cursor::new(wire);
    let mut dec = WebSocketFrameDecoder::new();
    let frame = match dec.step(&mut cursor).expect("step") {
        DecodeStep::Frame(f) => f,
        other => panic!("expected Frame, got {other:?}"),
    };
    assert_eq!(frame.payload, b"Hello");
    assert!(frame.mask.is_some());
    assert_eq!(frame.mask.unwrap(), mask);
}

/// 16-bit extended payload length (126 → 2-byte BE).
#[test]
fn extended_length_16bit() {
    let payload = vec![b'x'; 256];
    let mut wire = vec![0x82, 0x7E];
    wire.extend_from_slice(&(256u16).to_be_bytes());
    wire.extend_from_slice(&payload);

    let mut cursor = Cursor::new(wire);
    let mut dec = WebSocketFrameDecoder::new();
    let frame = match dec.step(&mut cursor).expect("step") {
        DecodeStep::Frame(f) => f,
        other => panic!("expected Frame, got {other:?}"),
    };
    assert_eq!(frame.opcode, Opcode::Binary);
    assert_eq!(frame.payload.len(), 256);
    assert_eq!(frame.payload, payload);
}

/// 64-bit extended payload length (127 → 8-byte BE).
#[test]
fn extended_length_64bit() {
    let payload = vec![b'z'; 70000];
    let mut wire = vec![0x81, 0x7F];
    wire.extend_from_slice(&(70000u64).to_be_bytes());
    wire.extend_from_slice(&payload);

    let mut cursor = Cursor::new(wire);
    let mut dec = WebSocketFrameDecoder::new();
    // 70000 bytes exceeds the default 8KB fill_from chunk — loop until Frame.
    let frame = loop {
        match dec.step(&mut cursor).expect("step") {
            DecodeStep::Frame(f) => break f,
            DecodeStep::Pending => continue,
        }
    };
    assert_eq!(frame.payload.len(), 70000);
    assert_eq!(frame.payload, payload);
}

/// Close frame (control frame, must have FIN=1).
#[test]
fn close_frame_decodes() {
    let wire: Vec<u8> = vec![0x88, 0x02, 0x03, 0xE8];
    let mut cursor = Cursor::new(wire);
    let mut dec = WebSocketFrameDecoder::new();
    let frame = match dec.step(&mut cursor).expect("step") {
        DecodeStep::Frame(f) => f,
        other => panic!("expected Frame, got {other:?}"),
    };
    assert_eq!(frame.opcode, Opcode::Close);
    assert!(frame.fin);
    assert_eq!(frame.payload, vec![0x03, 0xE8]);
}

/// has_partial reports true mid-frame.
#[test]
fn has_partial_mid_frame() {
    let mut dec = WebSocketFrameDecoder::new();
    assert!(!dec.has_partial());

    let buf = [0x81u8];
    let mut cursor = Cursor::new(&buf);
    let _ = dec.step(&mut cursor);
    assert!(dec.has_partial());

    let buf = [0x05, b'H', b'e', b'l', b'l', b'o'];
    let mut cursor = Cursor::new(&buf);
    let _ = dec.step(&mut cursor);
    assert!(!dec.has_partial());
}

/// Blocking decode wrapper produces same result as one-shot decode().
#[test]
fn blocking_wrapper_matches_decode() {
    let wire: Vec<u8> = vec![0x81, 0x05, b'H', b'e', b'l', b'l', b'o'];
    let mut cursor = Cursor::new(&wire);

    let frame = read_frame_blocking(&mut WebSocketFrameDecoder::new(), &mut cursor)
        .expect("decode")
        .expect("frame");

    assert_eq!(frame.payload, b"Hello");
}
