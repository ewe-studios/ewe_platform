//! `ArrowCodec` — columnar Arrow IPC via foundation_arrow (Decision 02).
//!
//! WHY: Arrow is the strongest zero-copy path — `RecordBatch` buffers are
//! Arc-backed and columnar, so a batch is `'static + Send + Sync` and read
//! column-wise with no per-field decode. It is a **platform extension** (only our
//! clients speak `application/arrow`), gated behind the `arrow` feature.
//!
//! WHAT: [`ArrowCodec`] with the [`CodecFor<M>`] blanket impl over
//! `ToArrow + FromArrow`, plus the inherent zero-copy [`ArrowCodec::unmarshal_batch`].
//!
//! HOW: `marshal` = a value → single-row `RecordBatch` → Arrow IPC bytes;
//! `unmarshal` = IPC bytes → `RecordBatch` → value. Arrow IPC is deterministic,
//! so `marshal_stable` reuses `marshal`.

use bytes::Bytes;
use foundation_arrow::arrow_array::RecordBatch;
use foundation_arrow::{decode_ipc, encode_ipc, FromArrow, ToArrow};

use super::{Codec, CodecError, CodecFor};

const NAME: &str = "arrow";

/// The columnar Arrow-IPC codec (`application/arrow` / `application/connect+arrow`).
#[derive(Debug, Clone, Copy, Default)]
pub struct ArrowCodec;

impl Codec for ArrowCodec {
    fn name(&self) -> &str {
        NAME
    }
    fn is_binary(&self) -> bool {
        true
    }
}

impl<M: ToArrow + FromArrow> CodecFor<M> for ArrowCodec {
    fn marshal(&self, message: &M) -> Result<Bytes, CodecError> {
        let batch = message
            .to_arrow()
            .map_err(|e| CodecError::encode(NAME, e.to_string()))?;
        let bytes = encode_ipc(&batch).map_err(|e| CodecError::encode(NAME, e.to_string()))?;
        Ok(Bytes::from(bytes))
    }

    fn unmarshal(&self, data: Bytes) -> Result<M, CodecError> {
        let batch = decode_ipc(&data).map_err(|e| CodecError::decode(NAME, e.to_string()))?;
        M::from_arrow(&batch).map_err(|e| CodecError::decode(NAME, e.to_string()))
    }

    fn marshal_stable(&self, message: &M) -> Result<Bytes, CodecError> {
        self.marshal(message)
    }

    fn marshal_append(&self, buf: &mut Vec<u8>, message: &M) -> Result<(), CodecError> {
        let bytes = self.marshal(message)?;
        buf.extend_from_slice(&bytes);
        Ok(())
    }
}

impl ArrowCodec {
    /// Zero-copy decode into an Arc-backed `RecordBatch` over `data` (Decision 02
    /// S4). Inherent (per-family return type), dispatched statically by the
    /// generated zero-copy variant.
    ///
    /// # Errors
    /// Returns [`CodecError::Decode`] if the bytes are not valid Arrow IPC.
    pub fn unmarshal_batch(&self, data: Bytes) -> Result<RecordBatch, CodecError> {
        decode_ipc(&data).map_err(|e| CodecError::decode(NAME, e.to_string()))
    }
}
