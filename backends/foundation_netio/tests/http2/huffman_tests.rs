//! Tests for `http2::hpack::huffman` — Huffman encode/decode round-trips
//! using the RFC 7541 Appendix B tables (Feature 29).

use bytes::BytesMut;
use foundation_netio::http2::hpack::huffman;

#[test]
fn decode_single_byte() {
    assert_eq!(&huffman::decode(&[0b00111111]).unwrap()[..], b"o");
    assert_eq!(&huffman::decode(&[7]).unwrap()[..], b"0");
    assert_eq!(&huffman::decode(&[(0x21 << 2) + 3]).unwrap()[..], b"A");
}

#[test]
fn single_char_multi_byte() {
    assert_eq!(&huffman::decode(&[255, 160 + 15]).unwrap()[..], b"#");
    assert_eq!(&huffman::decode(&[255, 200 + 7]).unwrap()[..], b"$");
}

#[test]
fn multi_char() {
    assert_eq!(&huffman::decode(&[254, 1]).unwrap()[..], b"!0");
    assert_eq!(
        &huffman::decode(&[0b01010011, 0b11111000]).unwrap()[..],
        b" !"
    );
}

#[test]
fn encode_decode_roundtrip() {
    const STRINGS: &[&str] = &[
        "hello world",
        ":method",
        ":path",
        ":authority",
        "example.com",
        "GET",
        "http",
        "text/html,application/xhtml+xml",
        "Mozilla/5.0",
        "Lorem ipsum dolor sit amet",
    ];
    for s in STRINGS {
        let mut dst = BytesMut::with_capacity(s.len());
        huffman::encode(s.as_bytes(), &mut dst);
        let decoded = huffman::decode(&dst).unwrap();
        assert_eq!(&decoded[..], s.as_bytes(), "roundtrip failed for: {s}");
    }
}

#[test]
fn encode_decode_binary() {
    const DATA: &[&[u8]] = &[b"\0", b"\0\0\0", b"\0\x01\x02\x03\x04\x05", b"\xFF\xF8"];
    for d in DATA {
        let mut dst = BytesMut::with_capacity(d.len());
        huffman::encode(d, &mut dst);
        let decoded = huffman::decode(&dst).unwrap();
        assert_eq!(&decoded[..], &d[..]);
    }
}
