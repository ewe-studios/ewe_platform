//! Tests for `http2::frame` — 9-byte header + all 10 frame types round-trip
//! encode/decode including error cases (Feature 29).

use bytes::{BufMut, Bytes, BytesMut};
use foundation_netio::http2::frame::*;

// ── Header ──────────────────────────────────────────────────────────────────

#[test]
fn frame_header_roundtrip() {
    let mut buf = BytesMut::new();
    let head = Head {
        kind: Kind::Headers,
        flag: headers_flags::END_HEADERS | headers_flags::END_STREAM,
        stream_id: 1,
    };
    head.encode(128, &mut buf);
    assert_eq!(buf.len(), 9);
    let mut arr = [0u8; 9];
    arr.copy_from_slice(&buf[..9]);
    let (parsed, plen) = Head::parse_with_len(&arr);
    assert_eq!(plen, 128);
    assert_eq!(parsed.kind, Kind::Headers);
    assert_eq!(parsed.stream_id, 1);
}

// ── DATA ────────────────────────────────────────────────────────────────────

#[test]
fn data_roundtrip() {
    let frame = DataFrame::new(5, Bytes::from_static(b"hello")).with_end_stream();
    let mut buf = BytesMut::new();
    frame.encode(&mut buf);
    let mut hdr = [0u8; 9];
    hdr.copy_from_slice(&buf[..9]);
    let (head, _) = Head::parse_with_len(&hdr);
    assert_eq!(head.kind, Kind::Data);
    let parsed = DataFrame::parse(&head, &buf[9..]).unwrap();
    assert_eq!(&parsed.data[..], b"hello");
    assert!(parsed.flags & data_flags::END_STREAM != 0);
}

// ── HEADERS ─────────────────────────────────────────────────────────────────

#[test]
fn headers_roundtrip() {
    let frame = HeadersFrame::new(3, vec![0x82]).with_end_stream();
    let mut buf = BytesMut::new();
    frame.encode(&mut buf);
    let mut hdr = [0u8; 9];
    hdr.copy_from_slice(&buf[..9]);
    let (head, _) = Head::parse_with_len(&hdr);
    assert_eq!(head.kind, Kind::Headers);
    let parsed = HeadersFrame::parse(&head, &buf[9..]).unwrap();
    assert_eq!(&parsed.header_block[..], &[0x82]);
    assert!(parsed.flags & headers_flags::END_HEADERS != 0);
    assert!(parsed.flags & headers_flags::END_STREAM != 0);
}

// ── SETTINGS ────────────────────────────────────────────────────────────────

#[test]
fn settings_roundtrip() {
    let frame = SettingsFrame::new(vec![
        Setting {
            id: SettingId::MaxConcurrentStreams,
            value: 100,
        },
        Setting {
            id: SettingId::InitialWindowSize,
            value: 65535,
        },
    ]);
    let mut buf = BytesMut::new();
    frame.encode(&mut buf);
    let mut hdr = [0u8; 9];
    hdr.copy_from_slice(&buf[..9]);
    let (head, _) = Head::parse_with_len(&hdr);
    assert_eq!(head.kind, Kind::Settings);
    assert_eq!(head.stream_id, 0);
    let parsed = SettingsFrame::parse(&head, &buf[9..]).unwrap();
    assert_eq!(parsed.settings.len(), 2);
    assert_eq!(parsed.settings[0].value, 100);
    assert_eq!(parsed.settings[1].value, 65535);
}

#[test]
fn settings_ack_roundtrip() {
    let frame = SettingsFrame::ack();
    let mut buf = BytesMut::new();
    frame.encode(&mut buf);
    let mut hdr = [0u8; 9];
    hdr.copy_from_slice(&buf[..9]);
    let (head, _) = Head::parse_with_len(&hdr);
    let parsed = SettingsFrame::parse(&head, &buf[9..]).unwrap();
    assert!(parsed.is_ack());
    assert!(parsed.settings.is_empty());
}

// ── PING ────────────────────────────────────────────────────────────────────

#[test]
fn ping_roundtrip() {
    let data = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let frame = PingFrame::new(data);
    let mut buf = BytesMut::new();
    frame.encode(&mut buf);
    let mut hdr = [0u8; 9];
    hdr.copy_from_slice(&buf[..9]);
    let (head, _) = Head::parse_with_len(&hdr);
    assert_eq!(head.kind, Kind::Ping);
    let parsed = PingFrame::parse(&head, &buf[9..]).unwrap();
    assert_eq!(parsed.opaque_data, data);
}

// ── GOAWAY ──────────────────────────────────────────────────────────────────

#[test]
fn goaway_roundtrip() {
    let frame = GoAwayFrame::new(7, ErrorCode::NoError);
    let mut buf = BytesMut::new();
    frame.encode(&mut buf);
    let mut hdr = [0u8; 9];
    hdr.copy_from_slice(&buf[..9]);
    let (head, _) = Head::parse_with_len(&hdr);
    assert_eq!(head.kind, Kind::GoAway);
    let parsed = GoAwayFrame::parse(&head, &buf[9..]).unwrap();
    assert_eq!(parsed.last_stream_id, 7);
    assert_eq!(parsed.error_code, ErrorCode::NoError);
}

// ── WINDOW_UPDATE ───────────────────────────────────────────────────────────

#[test]
fn window_update_roundtrip() {
    let frame = WindowUpdateFrame {
        stream_id: 0,
        size_increment: 65535,
    };
    let mut buf = BytesMut::new();
    frame.encode(&mut buf);
    let mut hdr = [0u8; 9];
    hdr.copy_from_slice(&buf[..9]);
    let (head, _) = Head::parse_with_len(&hdr);
    assert_eq!(head.kind, Kind::WindowUpdate);
    let parsed = WindowUpdateFrame::parse(&head, &buf[9..]).unwrap();
    assert_eq!(parsed.size_increment, 65535);
}

#[test]
fn window_update_zero_increment_is_error() {
    let mut buf = BytesMut::new();
    Head {
        kind: Kind::WindowUpdate,
        flag: 0,
        stream_id: 1,
    }
    .encode(4, &mut buf);
    buf.put_u32(0);
    let mut hdr = [0u8; 9];
    hdr.copy_from_slice(&buf[..9]);
    let (head, _) = Head::parse_with_len(&hdr);
    assert!(WindowUpdateFrame::parse(&head, &buf[9..]).is_err());
}

// ── RST_STREAM ──────────────────────────────────────────────────────────────

#[test]
fn reset_roundtrip() {
    let frame = ResetFrame {
        stream_id: 3,
        error_code: ErrorCode::Cancel,
    };
    let mut buf = BytesMut::new();
    frame.encode(&mut buf);
    let mut hdr = [0u8; 9];
    hdr.copy_from_slice(&buf[..9]);
    let (head, _) = Head::parse_with_len(&hdr);
    let parsed = ResetFrame::parse(&head, &buf[9..]).unwrap();
    assert_eq!(parsed.error_code, ErrorCode::Cancel);
}

// ── CONTINUATION ────────────────────────────────────────────────────────────

#[test]
fn continuation_roundtrip() {
    let frame = ContinuationFrame::new(1, vec![0x40, 0x01]);
    let mut buf = BytesMut::new();
    frame.encode(&mut buf);
    let mut hdr = [0u8; 9];
    hdr.copy_from_slice(&buf[..9]);
    let (head, _) = Head::parse_with_len(&hdr);
    let parsed = ContinuationFrame::parse(&head, &buf[9..]).unwrap();
    assert_eq!(&parsed.header_block[..], &[0x40, 0x01]);
}

// ── All 10 frame types ──────────────────────────────────────────────────────

#[test]
fn all_ten_kinds_roundtrip() {
    let frames: Vec<(Kind, usize)> = vec![
        (Kind::Data, 5),
        (Kind::Headers, 1),
        (Kind::Priority, 5),
        (Kind::Reset, 4),
        (Kind::Settings, 12),
        (Kind::PushPromise, 5),
        (Kind::Ping, 8),
        (Kind::GoAway, 8),
        (Kind::WindowUpdate, 4),
        (Kind::Continuation, 2),
    ];
    for (kind, payload_len) in &frames {
        let head = Head {
            kind: *kind,
            flag: 0,
            stream_id: 1,
        };
        let mut buf = BytesMut::new();
        head.encode(*payload_len as u32, &mut buf);
        buf.put_bytes(0xAA, *payload_len);
        let mut hdr = [0u8; 9];
        hdr.copy_from_slice(&buf[..9]);
        let (parsed, plen) = Head::parse_with_len(&hdr);
        assert_eq!(parsed.kind, *kind, "kind mismatch for {kind:?}");
        assert_eq!(
            plen, *payload_len as u32,
            "payload len mismatch for {kind:?}"
        );
    }
}
