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
        DecodeStep::Pending => panic!("expected Frame, got Pending"),
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
        DecodeStep::Pending => panic!("expected Frame, got Pending"),
    };
    assert_eq!(f1.payload, b"Hi");

    let f2 = match dec.step(&mut cursor).expect("step") {
        DecodeStep::Frame(f) => f,
        DecodeStep::Pending => panic!("expected Frame, got Pending"),
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
        DecodeStep::Pending => panic!("expected Frame, got Pending"),
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
        DecodeStep::Pending => panic!("expected Frame, got Pending"),
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
            DecodeStep::Pending => {}
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
        DecodeStep::Pending => panic!("expected Frame, got Pending"),
    };
    assert_eq!(frame.opcode, Opcode::Close);
    assert!(frame.fin);
    assert_eq!(frame.payload, vec![0x03, 0xE8]);
}

/// `has_partial` reports true mid-frame.
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

/// Blocking decode wrapper produces same result as one-shot `decode()`.
#[test]
fn blocking_wrapper_matches_decode() {
    let wire: Vec<u8> = vec![0x81, 0x05, b'H', b'e', b'l', b'l', b'o'];
    let mut cursor = Cursor::new(&wire);

    let frame = read_frame_blocking(&mut WebSocketFrameDecoder::new(), &mut cursor)
        .expect("decode")
        .expect("frame");

    assert_eq!(frame.payload, b"Hello");
}

#[test]
fn configured_limit_checks_the_declared_length_before_the_payload() {
    for header in [
        vec![0x82, 9],
        vec![0x82, 126, 1, 0],
        vec![0x82, 127, 0, 0, 0, 0, 0, 1, 0, 0],
    ] {
        let mut decoder = WebSocketFrameDecoder::with_limit(8);
        assert!(decoder.step(&mut header.as_slice()).is_err());
        assert_eq!(decoder.payload_got(), 0);
    }
}

#[test]
fn socket_eof_reports_truncated_frame_at_every_boundary() {
    use foundation_netio::websocket::WebSocketError;
    use std::io::ErrorKind;

    let wire = [0x81, 0x03, b'o', b'n', b'e'];
    for end in 0..wire.len() {
        let mut decoder = WebSocketFrameDecoder::new();
        assert!(matches!(
            decoder.read_frame(&mut &wire[..end]),
            Err(WebSocketError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof
        ));
    }
}

#[test]
fn socket_retries_preserve_partial_extended_length_mask_and_payload() {
    use foundation_netio::websocket::WebSocketError;
    use std::io::{self, ErrorKind, Read};
    struct Paused<'a>(&'a [u8], ErrorKind);
    impl Read for Paused<'_> {
        fn read(&mut self, target: &mut [u8]) -> io::Result<usize> {
            if self.0.is_empty() {
                return Err(self.1.into());
            }
            self.0.read(&mut target[..1])
        }
    }

    let payload = vec![b'x'; 126];
    let frame = foundation_netio::websocket::WebSocketFrame {
        fin: false,
        opcode: Opcode::Binary,
        mask: Some([1, 2, 3, 4]),
        payload,
    };
    let wire = frame.encode();
    for split in [0, 1, 2, 3, 4, 6, 8, 9, wire.len() - 1] {
        for kind in [ErrorKind::WouldBlock, ErrorKind::TimedOut] {
            let mut decoder = WebSocketFrameDecoder::new();
            assert!(matches!(
                decoder.read_frame(&mut Paused(&wire[..split], kind)),
                Err(WebSocketError::IoError(error)) if error.kind() == kind
            ));
            assert_eq!(decoder.read_frame(&mut &wire[split..]).unwrap(), frame);
        }
    }
}

#[test]
fn socket_delivers_close_frames_before_raw_eof() {
    use foundation_netio::websocket::{WebSocketError, WebSocketMessage};
    use std::io::ErrorKind;

    let mut stream: &[u8] = &[0x88, 0, 0x88, 4, 0x03, 0xe8, b'o', b'k'];
    let mut decoder = WebSocketFrameDecoder::new();
    for expected in [
        WebSocketMessage::Close(1005, String::new()),
        WebSocketMessage::Close(1000, "ok".into()),
    ] {
        assert_eq!(
            decoder
                .read_frame(&mut stream)
                .unwrap()
                .to_message()
                .unwrap(),
            expected
        );
    }
    assert!(matches!(
        decoder.read_frame(&mut stream),
        Err(WebSocketError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof
    ));
}
