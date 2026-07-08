//! `JsonCodec` — canonical protobuf-JSON via buffa + serde (Decision 02).
//!
//! WHY: JSON is the interop default alongside protobuf and is what browsers /
//! curl speak. buffa's generated messages carry serde impls that emit **canonical
//! protobuf-JSON** (lowerCamelCase, string enums, 64-bit ints as strings,
//! omit-zero, well-known-type formatting), so we serialize them with `serde_json`
//! directly and never hand-roll the mapping.
//!
//! WHAT: [`JsonCodec`] with the [`CodecFor<M>`] blanket impl over any
//! serde-serializable/deserializable type.
//!
//! The bound is **serde only** — not `buffa::Message`. The JSON path never
//! touches the protobuf methods, so requiring a proto schema would be spurious;
//! keeping it serde-only is what lets a JSON-only route
//! (`ProcedureCodecs::of((JsonCodec,))`) and a code-first `codecs(json)` service
//! work with **no protobuf/buffa involvement at all**. (buffa's generated
//! messages still qualify — they carry serde impls that emit canonical
//! protobuf-JSON — so proto+JSON tables via `defaults()` are unchanged.)
//!
//! HOW: `serde_json::to_vec` / `from_slice`. Zero-length payloads are rejected
//! (Decision 02 P16). `serde_json` output is already whitespace-free, so
//! `marshal_stable` reuses `marshal`.

use bytes::Bytes;
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::{Codec, CodecError, CodecFor};

const NAME: &str = "json";

/// The canonical protobuf-JSON codec (`application/json` / `application/connect+json`).
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonCodec;

impl Codec for JsonCodec {
    fn name(&self) -> &str {
        NAME
    }
    fn is_binary(&self) -> bool {
        false
    }
}

impl<M> CodecFor<M> for JsonCodec
where
    M: Serialize + DeserializeOwned,
{
    fn marshal(&self, message: &M) -> Result<Bytes, CodecError> {
        let bytes = serde_json::to_vec(message).map_err(|e| CodecError::encode(NAME, e.to_string()))?;
        Ok(Bytes::from(bytes))
    }

    fn unmarshal(&self, data: Bytes) -> Result<M, CodecError> {
        // Decision 02 P16: a zero-length body is not a valid JSON object.
        if data.is_empty() {
            return Err(CodecError::ZeroLength { codec: NAME });
        }
        serde_json::from_slice(&data).map_err(|e| CodecError::decode(NAME, e.to_string()))
    }

    fn marshal_stable(&self, message: &M) -> Result<Bytes, CodecError> {
        // serde_json emits compact (whitespace-free) output already — matching
        // connect-go's "serialize then compact" stable form.
        self.marshal(message)
    }

    fn marshal_append(&self, buf: &mut Vec<u8>, message: &M) -> Result<(), CodecError> {
        serde_json::to_writer(&mut *buf, message).map_err(|e| CodecError::encode(NAME, e.to_string()))
    }
}
