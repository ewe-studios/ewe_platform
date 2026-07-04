//! `ProcedureCodecs` — the single, handler-owned codec authority (Decision 02).
//!
//! WHY: The codec is chosen by the client per request via the `Content-Type`
//! wire token, so a procedure owns a *set* of codecs keyed by that token. There
//! is **no request-time registry**: name extraction is string mechanics, and
//! "is this codec supported here" is answered by this table. Built at
//! registration where `Req`/`Res` are concrete, then frozen.
//!
//! WHAT: [`ProcedureCodecs<Req, Res>`] (built via [`ProcedureCodecs::of`] /
//! [`defaults`](ProcedureCodecs::defaults) / [`only`](ProcedureCodecs::only) /
//! [`with_codec`](ProcedureCodecs::with_codec); resolved via
//! [`for_request`](ProcedureCodecs::for_request) /
//! [`for_response`](ProcedureCodecs::for_response)) and the [`CodecSet`] trait,
//! implemented for tuples of concrete codecs so a heterogeneous list stays fully
//! typed (no `Vec<Arc<dyn Codec>>` type-erasure wall).
//!
//! HOW: two `HashMap`s (request/response) keyed by wire name give O(1)
//! post-negotiation lookup; a miss is an error carrying the supported names
//! (rendered as HTTP 415 by the protocol layer), never a silent fallback.

use std::collections::HashMap;
use std::sync::Arc;

use buffa::Message;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{ConnectError, ConnectResult};

use super::{CodecFor, JsonCodec, ProtoCodec};

/// One built codec entry: the wire name plus the typed request/response handles
/// (two `Arc` handles over the *same* codec allocation, coerced to each side).
pub struct CodecEntry<Req, Res> {
    name: String,
    request: Arc<dyn CodecFor<Req>>,
    response: Arc<dyn CodecFor<Res>>,
}

/// A heterogeneous, fully-typed set of codecs supplied at once. Implemented for
/// tuples of concrete codecs (each element independently
/// `CodecFor<Req> + CodecFor<Res>`), so the list keeps its per-element types —
/// the exact thing a `Vec<Arc<dyn Codec>>` cannot do.
pub trait CodecSet<Req, Res> {
    /// Build the typed entries for this set.
    fn build(self) -> Vec<CodecEntry<Req, Res>>;
}

/// Build one entry: a single allocation, two coerced trait-object handles, name
/// read from the codec.
fn entry<Req, Res, C>(codec: C) -> CodecEntry<Req, Res>
where
    Req: 'static,
    Res: 'static,
    C: CodecFor<Req> + CodecFor<Res>,
{
    let shared = Arc::new(codec);
    let request: Arc<dyn CodecFor<Req>> = shared.clone();
    let response: Arc<dyn CodecFor<Res>> = shared;
    CodecEntry {
        name: request.name().to_string(),
        request,
        response,
    }
}

macro_rules! impl_codec_set {
    ($($idx:tt : $ty:ident),+) => {
        impl<Req, Res, $($ty),+> CodecSet<Req, Res> for ($($ty,)+)
        where
            Req: 'static,
            Res: 'static,
            $($ty: CodecFor<Req> + CodecFor<Res>,)+
        {
            fn build(self) -> Vec<CodecEntry<Req, Res>> {
                vec![ $( entry(self.$idx) ),+ ]
            }
        }
    };
}

impl_codec_set!(0: C0);
impl_codec_set!(0: C0, 1: C1);
impl_codec_set!(0: C0, 1: C1, 2: C2);
impl_codec_set!(0: C0, 1: C1, 2: C2, 3: C3);
impl_codec_set!(0: C0, 1: C1, 2: C2, 3: C3, 4: C4);
impl_codec_set!(0: C0, 1: C1, 2: C2, 3: C3, 4: C4, 5: C5);
impl_codec_set!(0: C0, 1: C1, 2: C2, 3: C3, 4: C4, 5: C5, 6: C6);
impl_codec_set!(0: C0, 1: C1, 2: C2, 3: C3, 4: C4, 5: C5, 6: C6, 7: C7);

/// Per-procedure, frozen codec table (Decision 02). Owned by the handler entry
/// (server) / client; nothing installs codecs after registration.
pub struct ProcedureCodecs<Req, Res> {
    request: HashMap<String, Arc<dyn CodecFor<Req>>>,
    response: HashMap<String, Arc<dyn CodecFor<Res>>>,
    /// Wire names in insertion order (for Accept-Post construction).
    names: Vec<String>,
}

impl<Req: 'static, Res: 'static> ProcedureCodecs<Req, Res> {
    /// Build from a set of concrete codecs supplied at once
    /// (`ProcedureCodecs::of((ProtoCodec, JsonCodec, ArrowCodec))`).
    #[must_use]
    pub fn of(set: impl CodecSet<Req, Res>) -> Self {
        let mut codecs = Self {
            request: HashMap::new(),
            response: HashMap::new(),
            names: Vec::new(),
        };
        for e in set.build() {
            codecs.insert(e);
        }
        codecs
    }

    /// A single-codec endpoint (`only(ArrowCodec)`): exactly one entry; every
    /// other content-type is a miss. Not Connect-conformant on its own — intended
    /// for our-stack extension endpoints, never the codegen default.
    #[must_use]
    pub fn only<C>(codec: C) -> Self
    where
        C: CodecFor<Req> + CodecFor<Res>,
    {
        Self::of((codec,))
    }

    /// Add a custom codec after construction (used by generated `*_with_codec`
    /// entry points). The wire name comes from [`Codec::name`](super::Codec::name).
    #[must_use]
    pub fn with_codec<C>(mut self, codec: C) -> Self
    where
        C: CodecFor<Req> + CodecFor<Res>,
    {
        self.insert(entry(codec));
        self
    }

    fn insert(&mut self, e: CodecEntry<Req, Res>) {
        if !self.request.contains_key(&e.name) {
            self.names.push(e.name.clone());
        }
        self.request.insert(e.name.clone(), e.request);
        self.response.insert(e.name, e.response);
    }

    /// O(1) request-codec resolution — the only lookup that exists. A miss
    /// carries the supported names (protocol layer renders HTTP 415), never a
    /// silent fallback.
    ///
    /// # Errors
    /// Returns an [`ConnectError`]-carrying trace when `name` is not in the table.
    pub fn for_request(&self, name: &str) -> ConnectResult<Arc<dyn CodecFor<Req>>> {
        self.request
            .get(name)
            .cloned()
            .ok_or_else(|| self.unsupported(name).into())
    }

    /// O(1) response-codec resolution (mirror of [`for_request`](Self::for_request)).
    ///
    /// # Errors
    /// Returns an [`ConnectError`]-carrying trace when `name` is not in the table.
    pub fn for_response(&self, name: &str) -> ConnectResult<Arc<dyn CodecFor<Res>>> {
        self.response
            .get(name)
            .cloned()
            .ok_or_else(|| self.unsupported(name).into())
    }

    /// The supported wire names in registration order (Accept-Post construction).
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    fn unsupported(&self, name: &str) -> ConnectError {
        ConnectError::unimplemented(format!(
            "unsupported codec {:?}, supported: [{}]",
            name,
            self.names.join(", ")
        ))
    }
}

impl<Req, Res> ProcedureCodecs<Req, Res>
where
    Req: Message + Serialize + DeserializeOwned + 'static,
    Res: Message + Serialize + DeserializeOwned + 'static,
{
    /// The interop default (what codegen emits): proto + json, per the Connect
    /// spec — callable by connect-go / connect-es / curl / browsers.
    ///
    /// The `buffa::Message + Serialize + DeserializeOwned` bound is what makes
    /// `defaults()` **reject a non-proto type at compile time** (an Arrow-only
    /// `ToArrow`/`FromArrow` type must use [`only`](Self::only) instead).
    #[must_use]
    pub fn defaults() -> Self {
        Self::of((ProtoCodec, JsonCodec))
    }
}
