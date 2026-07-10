//! HTTP/3 frame codec (RFC 9114 §7).
//!
//! WHY: frames arrive over QUIC streams in arbitrary chunks. The decoder must
//! resume across reads, treat a short read as `Pending` rather than an error, and
//! — critically — **ignore** frame types it does not know (RFC 9114 §9), or it
//! breaks against every newer peer.
//!
//! WHAT: round-trips, the incremental contract byte-by-byte, unknown/grease
//! frames, and the malformed cases that genuinely are protocol errors.

use bytes::Bytes;

use foundation_core::io::{DecodeStep, IncrementalDecoder};
use foundation_netio::http3::frame::{setting, ty, Frame, FrameDecoder};
use foundation_netio::http3::VarInt;

/// Decode every frame out of a complete buffer.
fn decode_all(bytes: &[u8]) -> Vec<Frame> {
    let mut decoder = FrameDecoder::new();
    let mut src = bytes;
    let mut frames = Vec::new();

    loop {
        match decoder.step(&mut src).expect("decode must not fail") {
            DecodeStep::Frame(f) => frames.push(f),
            DecodeStep::Pending => break,
        }
    }
    frames
}

/// One frame in, one frame out.
fn round_trip(frame: &Frame) {
    let encoded = frame.to_bytes();
    let decoded = decode_all(&encoded);
    assert_eq!(decoded, vec![frame.clone()], "round-trip of {frame:?}");
}

#[test]
fn every_frame_round_trips() {
    round_trip(&Frame::Data(Bytes::from_static(b"body bytes")));
    round_trip(&Frame::Headers(Bytes::from_static(b"\x00\x00qpack")));
    round_trip(&Frame::Settings(vec![
        (setting::QPACK_MAX_TABLE_CAPACITY, 0),
        (setting::MAX_FIELD_SECTION_SIZE, 16_384),
        (setting::QPACK_BLOCKED_STREAMS, 0),
    ]));
    round_trip(&Frame::GoAway(VarInt::new(12).unwrap()));
    round_trip(&Frame::MaxPushId(VarInt::new(100).unwrap()));
    round_trip(&Frame::CancelPush(VarInt::new(3).unwrap()));
    round_trip(&Frame::PushPromise {
        push_id: VarInt::new(7).unwrap(),
        encoded: Bytes::from_static(b"fields"),
    });
}

#[test]
fn empty_data_frame_round_trips() {
    // A zero-length DATA frame is legal and means "no bytes", not "end".
    round_trip(&Frame::Data(Bytes::new()));
    round_trip(&Frame::Settings(vec![]));
}

#[test]
fn several_frames_decode_from_one_buffer() {
    let mut buf = Vec::new();
    Frame::Settings(vec![(setting::MAX_FIELD_SECTION_SIZE, 4096)]).encode(&mut buf);
    Frame::Headers(Bytes::from_static(b"h")).encode(&mut buf);
    Frame::Data(Bytes::from_static(b"one")).encode(&mut buf);
    Frame::Data(Bytes::from_static(b"two")).encode(&mut buf);

    let frames = decode_all(&buf);
    assert_eq!(frames.len(), 4, "got {frames:?}");
    assert_eq!(frames[2], Frame::Data(Bytes::from_static(b"one")));
    assert_eq!(frames[3], Frame::Data(Bytes::from_static(b"two")));
}

#[test]
fn a_frame_split_across_every_byte_boundary_still_decodes() {
    // The whole point of `IncrementalDecoder`: a QUIC stream can hand us one byte
    // at a time, and a partial frame must be `Pending`, never an error.
    let frame = Frame::Headers(Bytes::from_static(b"a longer field section"));
    let encoded = frame.to_bytes();

    let mut decoder = FrameDecoder::new();
    let mut decoded = None;

    for (i, chunk) in encoded.chunks(1).enumerate() {
        let mut src = chunk;
        match decoder.step(&mut src).expect("no error mid-frame") {
            DecodeStep::Frame(f) => {
                assert_eq!(i, encoded.len() - 1, "the frame completes on the last byte");
                decoded = Some(f);
            }
            DecodeStep::Pending => {
                assert!(
                    decoder.has_partial() || i == 0,
                    "a decoder mid-frame must report partial state"
                );
            }
        }
    }

    assert_eq!(decoded, Some(frame));
    assert!(!decoder.has_partial(), "a fully consumed decoder holds nothing");
}

#[test]
fn unknown_frame_types_are_ignored_not_rejected() {
    // RFC 9114 §9: "Implementations MUST ignore unknown or unsupported values in
    // all extensible protocol elements." A decoder that errors here cannot talk to
    // a peer speaking any later draft.
    let unknown = Frame::Unknown { ty: 0x21, payload: Bytes::from_static(b"grease") };
    round_trip(&unknown);

    // And an unknown frame between two known ones must not disturb them.
    let mut buf = Vec::new();
    Frame::Headers(Bytes::from_static(b"h")).encode(&mut buf);
    unknown.encode(&mut buf);
    Frame::Data(Bytes::from_static(b"d")).encode(&mut buf);

    let frames = decode_all(&buf);
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0], Frame::Headers(Bytes::from_static(b"h")));
    assert!(matches!(frames[1], Frame::Unknown { ty: 0x21, .. }));
    assert_eq!(frames[2], Frame::Data(Bytes::from_static(b"d")));
}

#[test]
fn reserved_grease_frame_types_decode_as_unknown() {
    // RFC 9114 §7.2.8 reserves 0x1f * N + 0x21 to exercise the ignore path.
    for n in 0..4u64 {
        let ty = 0x1f * n + 0x21;
        let frame = Frame::Unknown { ty, payload: Bytes::from_static(b"") };
        let frames = decode_all(&frame.to_bytes());
        assert_eq!(frames.len(), 1, "grease type {ty:#x} must decode");
        assert_eq!(frames[0].ty(), ty);
    }
}

#[test]
fn control_only_frames_are_identified() {
    // The decoder surfaces these; the *connection* decides where they are legal,
    // because only it knows which stream the bytes came from.
    assert!(Frame::Settings(vec![]).is_control_only());
    assert!(Frame::GoAway(VarInt::new(0).unwrap()).is_control_only());
    assert!(Frame::MaxPushId(VarInt::new(0).unwrap()).is_control_only());
    assert!(Frame::CancelPush(VarInt::new(0).unwrap()).is_control_only());

    assert!(!Frame::Data(Bytes::new()).is_control_only());
    assert!(!Frame::Headers(Bytes::new()).is_control_only());

    assert!(Frame::Data(Bytes::new()).is_request_stream_frame());
    assert!(Frame::Headers(Bytes::new()).is_request_stream_frame());
    assert!(!Frame::Settings(vec![]).is_request_stream_frame());
}

#[test]
fn settings_with_a_repeated_identifier_is_a_protocol_error() {
    // RFC 9114 §7.2.4: a repeated setting identifier is H3_SETTINGS_ERROR.
    let mut payload = Vec::new();
    VarInt::new(setting::MAX_FIELD_SECTION_SIZE).unwrap().encode(&mut payload);
    VarInt::new(1).unwrap().encode(&mut payload);
    VarInt::new(setting::MAX_FIELD_SECTION_SIZE).unwrap().encode(&mut payload);
    VarInt::new(2).unwrap().encode(&mut payload);

    let mut buf = Vec::new();
    VarInt::new(ty::SETTINGS).unwrap().encode(&mut buf);
    VarInt::new(payload.len() as u64).unwrap().encode(&mut buf);
    buf.extend_from_slice(&payload);

    let mut decoder = FrameDecoder::new();
    let mut src = buf.as_slice();
    let err = decoder.step(&mut src).expect_err("a repeated identifier must be rejected");
    assert!(
        err.to_string().contains("repeats"),
        "the error must name the problem: {err}"
    );
}

#[test]
fn a_goaway_frame_with_trailing_bytes_is_rejected() {
    // GOAWAY's payload is exactly one varint. Anything else is malformed, and
    // silently ignoring the tail would hide a framing bug.
    let mut buf = Vec::new();
    VarInt::new(ty::GOAWAY).unwrap().encode(&mut buf);
    VarInt::new(3).unwrap().encode(&mut buf); // length 3
    buf.extend_from_slice(&[0x04, 0xff, 0xff]); // one varint + 2 junk bytes

    let mut decoder = FrameDecoder::new();
    let mut src = buf.as_slice();
    let err = decoder.step(&mut src).expect_err("trailing bytes must be rejected");
    assert!(err.to_string().contains("trailing"), "{err}");
}

#[test]
fn a_frame_larger_than_the_limit_is_rejected_without_buffering_it() {
    let mut buf = Vec::new();
    VarInt::new(ty::DATA).unwrap().encode(&mut buf);
    VarInt::new(1_000_000).unwrap().encode(&mut buf);
    // Deliberately do not append the payload: the decoder must reject on the
    // announced length alone, rather than trying to buffer a megabyte first.

    let mut decoder = FrameDecoder::new().with_max_frame_size(1024);
    let mut src = buf.as_slice();
    let err = decoder.step(&mut src).expect_err("oversized frame must be rejected");
    assert!(err.to_string().contains("exceeds"), "{err}");
}

#[test]
fn a_truncated_frame_reports_partial_state_at_eof() {
    // `has_partial` is how a caller distinguishes a clean end-of-stream from a
    // truncated frame when the QUIC stream FINs mid-frame.
    let frame = Frame::Data(Bytes::from_static(b"twelve bytes"));
    let encoded = frame.to_bytes();

    let mut decoder = FrameDecoder::new();
    let mut src = &encoded[..encoded.len() - 3];
    assert!(matches!(decoder.step(&mut src).expect("pending"), DecodeStep::Pending));
    assert!(decoder.has_partial(), "a truncated frame must be visible as partial");
}
