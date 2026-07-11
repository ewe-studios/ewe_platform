//! `ProtoCodec` — binary protobuf via buffa (Decision 02).
//!
//! WHY: Protobuf is the interop default (with JSON). buffa is a pure-Rust,
//! editions-first protobuf implementation with a zero-copy owning-view path.
//!
//! WHAT: [`ProtoCodec`] with the [`CodecFor<M>`] blanket impl over
//! `buffa::Message`, plus the inherent zero-copy [`ProtoCodec::unmarshal_owned_view`].
//!
//! HOW: `marshal` = `encode_to_bytes`, `unmarshal` = `decode_from_slice`,
//! `marshal_append` writes into the caller's buffer. buffa encodes fields in tag
//! order deterministically, so `marshal_stable` reuses `marshal` (Decision 02).

use bytes::Bytes;
use buffa::{Message, MessageView, OwnedView};

use super::{Codec, CodecError, CodecFor};

const NAME: &str = "proto";

/// The binary-protobuf codec (`application/proto` / `application/connect+proto`).
#[derive(Debug, Clone, Copy, Default)]
pub struct ProtoCodec;

impl Codec for ProtoCodec {
    fn name(&self) -> &str {
        NAME
    }
    fn is_binary(&self) -> bool {
        true
    }
}

impl<M: Message> CodecFor<M> for ProtoCodec {
    fn marshal(&self, message: &M) -> Result<Bytes, CodecError> {
        Ok(message.encode_to_bytes())
    }

    fn unmarshal(&self, data: Bytes) -> Result<M, CodecError> {
        M::decode_from_slice(&data).map_err(|e| CodecError::decode(NAME, e.to_string()))
    }

    fn marshal_stable(&self, message: &M) -> Result<Bytes, CodecError> {
        // buffa serializes fields in ascending tag order, which is already stable
        // for a fixed schema version — the property GET caching needs.
        Ok(message.encode_to_bytes())
    }

    fn marshal_append(&self, buf: &mut Vec<u8>, message: &M) -> Result<(), CodecError> {
        // `Vec<u8>` implements `bytes::BufMut`, so this appends without a copy.
        message.encode(buf);
        Ok(())
    }
}

impl ProtoCodec {
    /// Zero-copy decode into a `buffa::OwnedView<V>` backed by `bytes` (Decision
    /// 02 S4/RS2). The view is `'static + Send + Sync`, so it survives `.await`
    /// and crosses the pool with only an `Arc` refcount bump on the frame bytes.
    ///
    /// Inherent (not on `dyn CodecFor`) because the return type is per-family;
    /// the generated zero-copy handler variant dispatches it statically.
    ///
    /// # Errors
    /// Returns [`CodecError::Decode`] if the bytes are not valid protobuf.
    pub fn unmarshal_owned_view<V>(&self, bytes: Bytes) -> Result<OwnedView<V>, CodecError>
    where
        V: MessageView<'static>,
    {
        OwnedView::<V>::decode(bytes).map_err(|e| CodecError::decode(NAME, e.to_string()))
    }
}
