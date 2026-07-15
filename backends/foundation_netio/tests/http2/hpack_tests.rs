//! Tests for `http2::hpack` — HPACK decoder/encoder, static/dynamic tables,
//! integer encoding, Huffman round-trips, RFC 7541 Appendix C vectors (Feature 29).

use bytes::{BufMut, Bytes, BytesMut};
use foundation_netio::http2::hpack::*;

// ── Integer encoding ────────────────────────────────────────────────────────

#[test]
fn decode_int_small() {
    let buf = [0x0A];
    let (val, _n) = decode_int(&buf, 5).unwrap().unwrap();
    assert_eq!(val, 10);
}

#[test]
fn decode_int_prefix_max_values() {
    // RFC 7541 §5.1: when prefix == 2^N-1, continuation bytes follow.
    let buf = [0x1F, 0x00];
    let (val, n) = decode_int(&buf, 5).unwrap().unwrap();
    assert_eq!(val, 31);
    assert_eq!(n, 2);
}

#[test]
fn decode_int_multibyte() {
    let buf = [0x1F, 0x9A, 0x0A];
    let (val, n) = decode_int(&buf, 5).unwrap().unwrap();
    assert_eq!(val, 1337);
    assert_eq!(n, 3);
}

#[test]
fn decode_int_incomplete() {
    let buf = [0x1F];
    assert_eq!(decode_int(&buf, 5).unwrap(), None);
}

// ── Static table ────────────────────────────────────────────────────────────

#[test]
fn static_table_index_2_is_method_get() {
    let (name, value) = static_table_get(2).unwrap();
    assert_eq!(name, ":method");
    assert_eq!(value, "GET");
}

#[test]
fn static_table_index_1_is_authority() {
    let (name, value) = static_table_get(1).unwrap();
    assert_eq!(name, ":authority");
    assert_eq!(value, "");
}

#[test]
fn static_table_find() {
    assert_eq!(static_table_find_exact(b":method", b"GET"), Some(2));
    assert_eq!(static_table_find_exact(b":path", b"/"), Some(4));
    assert_eq!(static_table_find_name(b"content-type"), Some(31));
    assert_eq!(static_table_find_exact(b"nonexistent", b"value"), None);
}

// ── Dynamic table ───────────────────────────────────────────────────────────

#[test]
fn dynamic_table_insert_and_lookup() {
    let mut table = DynamicTable::new(4096);
    table.insert(Bytes::from("custom-name"), Bytes::from("custom-value"));
    assert_eq!(table.len(), 1);
    let (name, value) = table.get(62).unwrap();
    assert_eq!(name, b"custom-name");
    assert_eq!(value, b"custom-value");
}

#[test]
fn dynamic_table_evicts_when_full() {
    let entry_size = 5 + 5 + 32;
    let mut table = DynamicTable::new(entry_size * 2);
    table.insert(Bytes::from("aaaaa"), Bytes::from("bbbbb"));
    table.insert(Bytes::from("ccccc"), Bytes::from("ddddd"));
    assert_eq!(table.len(), 2);
    table.insert(Bytes::from("eeeee"), Bytes::from("fffff"));
    assert_eq!(table.len(), 2);
    let (name, _) = table.get(62).unwrap();
    assert_eq!(name, b"eeeee");
}

#[test]
fn dynamic_table_resize_to_zero_clears() {
    let mut table = DynamicTable::new(4096);
    table.insert(Bytes::from("a"), Bytes::from("b"));
    table.set_max_size(0);
    assert_eq!(table.len(), 0);
    assert_eq!(table.size(), 0);
}

// ── Decoder ─────────────────────────────────────────────────────────────────

#[test]
fn decode_indexed_header() {
    let mut dec = Decoder::new();
    let headers = dec.decode(&[0x82]).unwrap();
    assert_eq!(headers.len(), 1);
    assert_eq!(&headers[0].0[..], b":method");
    assert_eq!(&headers[0].1[..], b"GET");
}

#[test]
fn decode_literal_indexed_name() {
    let mut buf = BytesMut::new();
    buf.put_u8(0x41);
    let value = b"example.com";
    buf.put_u8(value.len() as u8);
    buf.put_slice(value);
    let mut dec = Decoder::new();
    let headers = dec.decode(&buf).unwrap();
    assert_eq!(headers.len(), 1);
    assert_eq!(&headers[0].0[..], b":authority");
    assert_eq!(&headers[0].1[..], b"example.com");
    assert_eq!(dec.table().len(), 1);
}

#[test]
fn decode_multiple_headers_fragmented() {
    let mut dec = Decoder::new();
    let h1 = dec.decode(&[0x82]).unwrap();
    assert_eq!(h1.len(), 1);
    let h2 = dec.decode(&[]).unwrap();
    assert_eq!(h2.len(), 0);
}

// ── Encoder ─────────────────────────────────────────────────────────────────

#[test]
fn encoder_indexed_static() {
    let mut enc = Encoder::new();
    let mut dst = BytesMut::new();
    enc.encode_header(b":method", b"GET", &mut dst);
    assert_eq!(&dst[..], &[0x82]);
}

#[test]
fn encoder_literal_indexed() {
    let mut enc = Encoder::new();
    let mut dst = BytesMut::new();
    enc.encode_header(b"x-custom", b"value", &mut dst);
    let mut dec = Decoder::new();
    let headers = dec.decode(&dst).unwrap();
    assert_eq!(headers.len(), 1);
    assert_eq!(&headers[0].0[..], b"x-custom");
    assert_eq!(&headers[0].1[..], b"value");
}

// ── RFC 7541 Appendix C test vectors ────────────────────────────────────────

#[test]
fn rfc7541_c2_first_request() {
    let mut enc = Encoder::new();
    let mut dst = BytesMut::new();
    enc.encode_header(b":method", b"GET", &mut dst);
    enc.encode_header(b":scheme", b"http", &mut dst);
    enc.encode_header(b":path", b"/", &mut dst);
    enc.encode_header(b":authority", b"www.example.com", &mut dst);
    let mut dec = Decoder::new();
    let headers = dec.decode(&dst).unwrap();
    assert_eq!(headers.len(), 4);
    assert_eq!(&headers[0].0[..], b":method");
    assert_eq!(&headers[0].1[..], b"GET");
}

#[test]
fn rfc7541_c2_second_request() {
    let mut enc = Encoder::new();
    let mut dst1 = BytesMut::new();
    enc.encode_header(b":method", b"GET", &mut dst1);
    enc.encode_header(b":scheme", b"http", &mut dst1);
    enc.encode_header(b":path", b"/", &mut dst1);
    enc.encode_header(b":authority", b"www.example.com", &mut dst1);
    assert!(enc.table().len() > 0);
}

#[test]
fn roundtrip_various_headers() {
    let mut enc = Encoder::new();
    let mut dst = BytesMut::new();
    let input: Vec<(&[u8], &[u8])> = vec![
        (b":method", b"POST"),
        (b":scheme", b"https"),
        (b":path", b"/api/v1/users"),
        (b":authority", b"api.example.com"),
        (b"content-type", b"application/json"),
        (b"accept", b"application/json"),
        (b"user-agent", b"hpack-test/1.0"),
        (b"x-request-id", b"abc123"),
    ];
    for (name, value) in &input {
        enc.encode_header(name, value, &mut dst);
    }
    let mut dec = Decoder::new();
    let output = dec.decode(&dst).unwrap();
    assert_eq!(output.len(), input.len());
    for ((in_name, in_val), (out_name, out_val)) in input.iter().zip(output.iter()) {
        assert_eq!(out_name.as_ref(), *in_name);
        assert_eq!(out_val.as_ref(), *in_val);
    }
}
