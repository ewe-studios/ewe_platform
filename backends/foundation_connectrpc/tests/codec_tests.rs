//! Codec-system tests (spec-41 F13 / Decision 02): proto/json round-trips,
//! zero-length rejection, `marshal_append`, `CodecError` mapping, and the
//! `ProcedureCodecs` table (of/defaults/only/with_codec/for_request/names + miss).
//!
//! Uses a hand-written but real buffa `Message` (`TestMsg`) so proto encoding
//! carries actual data, plus serde derives so `JsonCodec` exercises the same
//! serde path buffa's canonical JSON uses. (`compute_size` only presizes the
//! buffer in buffa — `write_to`/`merge_field` are the correctness surface.)

use buffa::encoding::{decode_varint, encode_varint, skip_field, Tag, WireType};
use buffa::{DecodeContext, DecodeError, DefaultInstance, Message, SizeCache};
use bytes::{Buf, BufMut, Bytes};

use foundation_connectrpc::shared::codec::{Codec, CodecError, CodecFor};
use foundation_connectrpc::{Code, JsonCodec, ProcedureCodecs, ProtoCodec};
use foundation_errstacks::ErrorTrace;

// ── A real, minimal buffa message (id: int32 @1, name: string @2) ─────────────

#[derive(Clone, Default, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
struct TestMsg {
    id: i32,
    name: String,
}

impl DefaultInstance for TestMsg {
    fn default_instance() -> &'static Self {
        static INST: std::sync::OnceLock<TestMsg> = std::sync::OnceLock::new();
        INST.get_or_init(TestMsg::default)
    }
}

impl Message for TestMsg {
    fn compute_size(&self, _cache: &mut SizeCache) -> u32 {
        // Presize hint only; BufMut in write_to grows as needed.
        0
    }

    fn write_to(&self, _cache: &mut SizeCache, buf: &mut impl BufMut) {
        Tag::new(1, WireType::Varint).encode(buf);
        encode_varint(self.id as u64, buf);
        Tag::new(2, WireType::LengthDelimited).encode(buf);
        encode_varint(self.name.len() as u64, buf);
        buf.put_slice(self.name.as_bytes());
    }

    fn merge_field(
        &mut self,
        tag: Tag,
        buf: &mut impl Buf,
        _ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        match tag.field_number() {
            1 => {
                self.id = decode_varint(buf)? as i32;
                Ok(())
            }
            2 => {
                let len = decode_varint(buf)? as usize;
                let mut bytes = vec![0u8; len];
                buf.copy_to_slice(&mut bytes);
                self.name = String::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
                Ok(())
            }
            _ => skip_field(tag, buf),
        }
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

// ── Proto codec ───────────────────────────────────────────────────────────────

#[test]
fn proto_marshal_unmarshal_roundtrip() {
    let codec = ProtoCodec;
    let msg = TestMsg {
        id: 7,
        name: "hello".to_string(),
    };
    let bytes = CodecFor::<TestMsg>::marshal(&codec, &msg).expect("marshal");
    assert!(!bytes.is_empty(), "proto encoding should carry the fields");
    let back: TestMsg = codec.unmarshal(bytes).expect("unmarshal");
    assert_eq!(back, msg);
    assert_eq!(codec.name(), "proto");
    assert!(codec.is_binary());
}

#[test]
fn proto_marshal_append_matches_marshal() {
    let codec = ProtoCodec;
    let msg = TestMsg {
        id: 42,
        name: "x".to_string(),
    };
    let direct = CodecFor::<TestMsg>::marshal(&codec, &msg).unwrap();
    let mut buf = Vec::new();
    codec.marshal_append(&mut buf, &msg).unwrap();
    assert_eq!(buf, direct.to_vec());
}

// ── JSON codec (canonical serde path) ─────────────────────────────────────────

#[test]
fn json_marshal_unmarshal_roundtrip() {
    let codec = JsonCodec;
    let msg = TestMsg {
        id: 7,
        name: "hi".to_string(),
    };
    let bytes = CodecFor::<TestMsg>::marshal(&codec, &msg).unwrap();
    assert_eq!(&bytes[..], br#"{"id":7,"name":"hi"}"#);
    let back: TestMsg = codec.unmarshal(bytes).unwrap();
    assert_eq!(back, msg);
    assert_eq!(codec.name(), "json");
    assert!(!codec.is_binary());
}

#[test]
fn json_zero_length_is_rejected() {
    let codec = JsonCodec;
    let err = CodecFor::<TestMsg>::unmarshal(&codec, Bytes::new()).unwrap_err();
    assert!(matches!(err, CodecError::ZeroLength { codec: "json" }));
    // Maps to InvalidArgument at the RPC boundary.
    let trace: ErrorTrace<_> = err.into();
    assert_eq!(trace.current_context().code(), Code::InvalidArgument);
}

#[test]
fn json_decode_error_maps_to_invalid_argument() {
    let codec = JsonCodec;
    let err = CodecFor::<TestMsg>::unmarshal(&codec, Bytes::from_static(b"not json")).unwrap_err();
    assert!(matches!(err, CodecError::Decode { codec: "json", .. }));
    let connect: foundation_connectrpc::ConnectError = err.into();
    assert_eq!(connect.code(), Code::InvalidArgument);
}

// ── ProcedureCodecs table ─────────────────────────────────────────────────────

#[test]
fn defaults_has_proto_and_json_and_rejects_arrow() {
    let codecs = ProcedureCodecs::<TestMsg, TestMsg>::defaults();
    assert!(codecs.for_request("proto").is_ok());
    assert!(codecs.for_request("json").is_ok());
    assert!(codecs.for_response("json").is_ok());
    // A codec not in the table is a miss (rendered 415 by the protocol layer).
    let miss = codecs.for_request("arrow").err().expect("arrow is unsupported");
    assert_eq!(miss.current_context().code(), Code::Unimplemented);
    let names: Vec<&str> = codecs.names().collect();
    assert_eq!(names, vec!["proto", "json"]);
}

#[test]
fn resolved_codec_marshals() {
    let codecs = ProcedureCodecs::<TestMsg, TestMsg>::defaults();
    let json = codecs.for_request("json").unwrap();
    let bytes = json
        .marshal(&TestMsg {
            id: 1,
            name: "a".into(),
        })
        .unwrap();
    assert_eq!(&bytes[..], br#"{"id":1,"name":"a"}"#);
}

#[test]
fn only_is_single_codec() {
    let codecs = ProcedureCodecs::<TestMsg, TestMsg>::only(ProtoCodec);
    assert!(codecs.for_request("proto").is_ok());
    assert!(codecs.for_request("json").is_err());
    assert_eq!(codecs.names().collect::<Vec<_>>(), vec!["proto"]);
}

#[test]
fn of_tuple_and_with_codec() {
    // `of` with an explicit tuple, then add another via with_codec (idempotent name).
    let codecs = ProcedureCodecs::<TestMsg, TestMsg>::of((ProtoCodec,)).with_codec(JsonCodec);
    assert!(codecs.for_request("proto").is_ok());
    assert!(codecs.for_request("json").is_ok());
    assert_eq!(codecs.names().collect::<Vec<_>>(), vec!["proto", "json"]);
}
