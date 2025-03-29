#![allow(clippy::items_after_test_module)]
#![allow(clippy::missing_doc_code_examples)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]

use std::error::Error;

/// OpIndex defines the different operations that is
/// representable and performable through the underlying
/// index returned by the op_index method which returns
/// the first byte indicative of the type of operation in a
/// 32 bit number.
///
/// This allows us squeeze varying facts about a desired
/// operation to be performed into a single u32 (32 bits)
/// or in the future (u64) 64 bit number.
pub trait OpIndex {
    fn op_index(&self) -> u8;
}

macro_rules! define_index {
    ($id:literal,$item:ty) => {
        impl OpIndex for $item {
            fn op_index(&self) -> u8 {
                $id
            }
        }
    };
}

/// [`Batchable`] defines a infallible type which can be
/// encoded into a [`BatchEncodable`] implementing type
/// usually a [`Batch`].
pub trait Batchable {
    fn encode(encoder: impl BatchEncodable);
}

// [`CallParams`] defines the underlying location of memory
// indicative of the starting pointer and length for which it
// relates to.
#[allow(unused)]
pub struct CallParams(pub *const u8, pub u32);

impl CallParams {
    #[must_use]
    pub fn new(addr: *const u8, length: u32) -> Self {
        Self(addr, length)
    }
}

/// ExternalPointer identifies an external handle pointing
/// to a special pointer that is used to control/access
/// an external resource.
///
/// Can be an object, function or some other resource
/// that is to be used across wasm boundaries.
pub struct ExternalPointer(u64);

impl ExternalPointer {
    fn into_inner(self) -> u64 {
        self.0
    }

    fn borrow<'a>(&'a self) -> &'a u64 {
        &self.0
    }

    fn clone_inner(&self) -> u64 {
        self.0.clone()
    }
}

pub enum Params<'a> {
    Undefined,
    Null,
    Float64(f64),
    BigInt(i64),
    String(&'a str),
    Float32Array(&'a [f32]),
    Float64Array(&'a [f64]),
    Bool(bool),
    Uint32Array(&'a [u32]),
    ExternalReference(&'a ExternalPointer),
}

impl From<f64> for Params<'_> {
    fn from(f: f64) -> Self {
        Params::Float64(f)
    }
}

impl From<i32> for Params<'_> {
    fn from(i: i32) -> Self {
        Params::Float64(f64::from(i))
    }
}

impl From<usize> for Params<'_> {
    fn from(i: usize) -> Self {
        Params::Float64(i as f64)
    }
}

impl From<i64> for Params<'_> {
    fn from(i: i64) -> Self {
        Params::BigInt(i)
    }
}

impl<'a> From<&'a str> for Params<'a> {
    fn from(s: &'a str) -> Self {
        Params::String(s)
    }
}

impl<'a> From<&'a ExternalPointer> for Params<'a> {
    fn from(i: &'a ExternalPointer) -> Self {
        Params::ExternalReference(i)
    }
}

impl<'a> From<&'a [f32]> for Params<'a> {
    fn from(a: &'a [f32]) -> Self {
        Params::Float32Array(a)
    }
}

impl<'a> From<&'a [f64]> for Params<'a> {
    fn from(a: &'a [f64]) -> Self {
        Params::Float64Array(a)
    }
}

impl From<bool> for Params<'_> {
    fn from(b: bool) -> Self {
        Params::Bool(b)
    }
}

impl<'a> From<&'a [u32]> for Params<'a> {
    fn from(a: &'a [u32]) -> Self {
        Params::Uint32Array(a)
    }
}

/// [`FuncHandle`] defines a type alias providing more
/// context that this is used to represent the locaiton
/// of a runtime function across WASM boundary.
pub type FuncHandle = u32;

/// FuncCall defines the different function calls
/// we can possible make which should be supported
/// by whatever underlying runtime environment
/// gets the underlying binary representation
/// without any form of deserialization efforts
/// as this will effectively be represented in binary
pub enum FuncCall {
    NoReturnFunc(FuncHandle, CallParams),
    I64ReturnFunc(FuncHandle, CallParams),
    I32ReturnFunc(FuncHandle, CallParams),
    U32ReturnFunc(FuncHandle, CallParams),
    U64ReturnFunc(FuncHandle, CallParams),
    StringReturnFunc(FuncHandle, CallParams),
    ObjectReturnFunc(FuncHandle, CallParams),
}

define_index!(1, FuncCall);

#[cfg(test)]
mod test_func_call {
    use crate::ops::{CallParams, FuncCall, OpIndex};

    #[test]
    fn can_get_func_call() {
        let handler = FuncCall::NoReturnFunc(0, CallParams::new(&0, 10));
        assert_eq!(handler.op_index(), 1);
    }
}

/// [`StrLocation`] represent the underlying location of an
/// encoded string which points to the relevant address
/// of the string and it's underlying starting index and
/// length from that index location.
pub struct StrLocation(pub *const u8, pub u8, pub u8);

/// [`BatchEncodable`] defines a trait which allows you implement
/// conversion an underlying binary representation of a Batch
/// operation.
pub trait BatchEncodable {
    /// [`string`] encodes the underlying string
    /// returning the string location information which allows
    /// whatever is calling it
    fn string(&self, data: &str) -> StrLocation;

    /// [`op`] provides new operation data to be encoded
    /// into the underlying data stream.
    fn op(&self, data: &[u8]);
}

pub struct Batch {
    pub text: Vec<u8>,
    pub ops: Vec<u8>,
}

pub enum Ops {
    Func(FuncCall),
}

pub struct Operations(Vec<Ops>);
