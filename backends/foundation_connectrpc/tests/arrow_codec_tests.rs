//! Arrow-codec tests (spec-41 F13 / Decision 02), gated on the `arrow` feature.
//!
//! Verifies the tri-family acceptance: a single type that is a buffa `Message`
//! (+ serde) **and** `ToArrow + FromArrow` can be served by all three codecs via
//! `ProcedureCodecs::of((ProtoCodec, JsonCodec, ArrowCodec))`, and that the Arrow
//! codec round-trips through Arrow IPC plus the zero-copy `unmarshal_batch`.
#![cfg(feature = "arrow")]

use buffa::encoding::{decode_varint, encode_varint, skip_field, Tag, WireType};
use buffa::{DecodeContext, DecodeError, DefaultInstance, Message, SizeCache};
use bytes::{Buf, BufMut};

use foundation_arrow::arrow_array::{Array, Int32Array, RecordBatch};
use foundation_arrow::arrow_schema::{DataType, Field, Schema};
use foundation_arrow::{ArrowSchema, FromArrow, IpcResult, ToArrow};
use std::sync::Arc;

use foundation_connectrpc::shared::codec::CodecFor;
use foundation_connectrpc::{ArrowCodec, JsonCodec, ProcedureCodecs, ProtoCodec};

/// A tri-family message: buffa `Message` (+ serde) and Arrow (`v: int32`).
#[derive(Clone, Default, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
struct Tri {
    v: i32,
}

impl DefaultInstance for Tri {
    fn default_instance() -> &'static Self {
        static INST: std::sync::OnceLock<Tri> = std::sync::OnceLock::new();
        INST.get_or_init(Tri::default)
    }
}

impl Message for Tri {
    fn compute_size(&self, _c: &mut SizeCache) -> u32 {
        0
    }
    fn write_to(&self, _c: &mut SizeCache, buf: &mut impl BufMut) {
        Tag::new(1, WireType::Varint).encode(buf);
        encode_varint(self.v as u64, buf);
    }
    fn merge_field(
        &mut self,
        tag: Tag,
        buf: &mut impl Buf,
        _ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        if tag.field_number() == 1 {
            self.v = decode_varint(buf)? as i32;
            Ok(())
        } else {
            skip_field(tag, buf)
        }
    }
    fn clear(&mut self) {
        *self = Self::default();
    }
}

impl ArrowSchema for Tri {
    fn schema() -> Schema {
        Schema::new(Self::fields())
    }
    fn fields() -> Vec<Field> {
        vec![Field::new("v", DataType::Int32, false)]
    }
}

impl ToArrow for Tri {
    fn to_arrow(&self) -> IpcResult<RecordBatch> {
        RecordBatch::try_new(Self::schema_ref(), vec![Arc::new(Int32Array::from(vec![self.v]))])
    }
    fn to_arrow_batch(values: &[Self]) -> IpcResult<RecordBatch> {
        let col = Int32Array::from(values.iter().map(|t| t.v).collect::<Vec<_>>());
        RecordBatch::try_new(Self::schema_ref(), vec![Arc::new(col)])
    }
}

impl FromArrow for Tri {
    fn from_arrow(batch: &RecordBatch) -> IpcResult<Self> {
        let col = batch
            .column(0)
            .as_any()
            .downcast_ref::<Int32Array>()
            .expect("int32 column");
        Ok(Self { v: col.value(0) })
    }
    fn from_arrow_batch(batch: &RecordBatch) -> IpcResult<Vec<Self>> {
        let col = batch
            .column(0)
            .as_any()
            .downcast_ref::<Int32Array>()
            .expect("int32 column");
        Ok((0..col.len()).map(|i| Self { v: col.value(i) }).collect())
    }
}

#[test]
fn of_all_three_families_compiles_and_resolves() {
    // The acceptance: one type served by proto + json + arrow at once.
    let codecs = ProcedureCodecs::<Tri, Tri>::of((ProtoCodec, JsonCodec, ArrowCodec));
    assert!(codecs.for_request("proto").is_ok());
    assert!(codecs.for_request("json").is_ok());
    assert!(codecs.for_request("arrow").is_ok());
    assert_eq!(
        codecs.names().collect::<Vec<_>>(),
        vec!["proto", "json", "arrow"]
    );
}

#[test]
fn arrow_codec_roundtrip_and_batch() {
    let codec = ArrowCodec;
    let msg = Tri { v: 99 };
    let bytes = CodecFor::<Tri>::marshal(&codec, &msg).expect("marshal");
    let back: Tri = codec.unmarshal(bytes.clone()).expect("unmarshal");
    assert_eq!(back, msg);

    // Inherent zero-copy path: Arc-backed RecordBatch over the frame bytes.
    let batch = codec.unmarshal_batch(bytes).expect("batch");
    let col = batch
        .column(0)
        .as_any()
        .downcast_ref::<Int32Array>()
        .unwrap();
    assert_eq!(col.value(0), 99);
}
