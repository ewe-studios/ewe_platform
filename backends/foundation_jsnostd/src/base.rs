/// [`ValueTypes`] represent the underlying type of value
/// being encoded into a binary stream.
#[repr(usize)]
pub enum ValueTypes {
    Null = 0,
    Undefined = 1,
    Bool = 2,
    Text8 = 3,
    Text16 = 4,
    Int8 = 5,
    Int16 = 6,
    Int32 = 7,
    Int64 = 8,
    Uint8 = 9,
    Uint16 = 10,
    Uint32 = 11,
    Uint64 = 12,
    Float32 = 13,
    Float64 = 14,
    ExternalReference = 15,
    Uint8ArrayBuffer = 16,
    Uint16ArrayBuffer = 17,
    Uint32ArrayBuffer = 18,
    Uint64ArrayBuffer = 19,
    Int8ArrayBuffer = 20,
    Int16ArrayBuffer = 21,
    Int32ArrayBuffer = 22,
    Int64ArrayBuffer = 23,
    Float32ArrayBuffer = 24,
    Float64ArrayBuffer = 25,
    InternalReference = 26,
}

#[allow(clippy::from_over_into)]
impl Into<u8> for ValueTypes {
    fn into(self) -> u8 {
        self as u8
    }
}

pub enum Params<'a> {
    Undefined,
    Null,
    Bool(bool),
    Float32(f32),
    Float64(f64),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Uint8(u8),
    Uint16(u16),
    Uint32(u32),
    Uint64(u64),
    Text8(&'a str),
    Text16(&'a [u16]),
    Int8Array(&'a [i8]),
    Int16Array(&'a [i16]),
    Int32Array(&'a [i32]),
    Int64Array(&'a [i64]),
    Uint8Array(&'a [u8]),
    Uint16Array(&'a [u16]),
    Uint32Array(&'a [u32]),
    Uint64Array(&'a [u64]),
    Float32Array(&'a [f32]),
    Float64Array(&'a [f64]),
    ExternalReference(&'a ExternalPointer),
    InternalReference(&'a InternalPointer),
}

impl Params<'_> {
    pub fn to_value_type(&self) -> ValueTypes {
        match self {
            Params::Bool(_) => ValueTypes::Bool,
            Params::Undefined => ValueTypes::Undefined,
            Params::Null => ValueTypes::Null,
            Params::Float32(_) => ValueTypes::Float32,
            Params::Float64(_) => ValueTypes::Float64,
            Params::Int8(_) => ValueTypes::Int8,
            Params::Int16(_) => ValueTypes::Int16,
            Params::Int32(_) => ValueTypes::Int32,
            Params::Int64(_) => ValueTypes::Int64,
            Params::Uint8(_) => ValueTypes::Uint8,
            Params::Uint16(_) => ValueTypes::Uint16,
            Params::Uint64(_) => ValueTypes::Uint64,
            Params::Uint32(_) => ValueTypes::Uint32,
            Params::Text8(_) => ValueTypes::Text8,
            Params::Text16(_) => ValueTypes::Text16,
            Params::Int8Array(_) => ValueTypes::Int8ArrayBuffer,
            Params::Int16Array(_) => ValueTypes::Int16ArrayBuffer,
            Params::Int32Array(_) => ValueTypes::Int32ArrayBuffer,
            Params::Int64Array(_) => ValueTypes::Int64ArrayBuffer,
            Params::Uint8Array(_) => ValueTypes::Uint8ArrayBuffer,
            Params::Uint16Array(_) => ValueTypes::Uint16ArrayBuffer,
            Params::Uint32Array(_) => ValueTypes::Uint32ArrayBuffer,
            Params::Uint64Array(_) => ValueTypes::Uint64ArrayBuffer,
            Params::Float32Array(_) => ValueTypes::Float32ArrayBuffer,
            Params::Float64Array(_) => ValueTypes::Float64ArrayBuffer,
            Params::ExternalReference(_) => ValueTypes::ExternalReference,
            Params::InternalReference(_) => ValueTypes::InternalReference,
        }
    }
}

impl From<f64> for Params<'_> {
    fn from(f: f64) -> Self {
        Params::Float64(f)
    }
}

impl From<u8> for Params<'_> {
    fn from(i: u8) -> Self {
        Params::Uint8(i)
    }
}

impl From<u16> for Params<'_> {
    fn from(i: u16) -> Self {
        Params::Uint16(i)
    }
}

impl From<u32> for Params<'_> {
    fn from(i: u32) -> Self {
        Params::Uint32(i)
    }
}

impl From<u64> for Params<'_> {
    fn from(i: u64) -> Self {
        Params::Uint64(i)
    }
}

impl From<i8> for Params<'_> {
    fn from(i: i8) -> Self {
        Params::Int8(i)
    }
}

impl From<i16> for Params<'_> {
    fn from(i: i16) -> Self {
        Params::Int16(i)
    }
}

impl From<i32> for Params<'_> {
    fn from(i: i32) -> Self {
        Params::Int32(i)
    }
}

impl From<i64> for Params<'_> {
    fn from(i: i64) -> Self {
        Params::Int64(i)
    }
}

impl From<usize> for Params<'_> {
    fn from(i: usize) -> Self {
        Params::Float64(i as f64)
    }
}

impl<'a> From<&'a str> for Params<'a> {
    fn from(s: &'a str) -> Self {
        Params::Text8(s)
    }
}

impl<'a> From<&'a InternalPointer> for Params<'a> {
    fn from(i: &'a InternalPointer) -> Self {
        Params::InternalReference(i)
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

impl<'a> From<&'a [i8]> for Params<'a> {
    fn from(a: &'a [i8]) -> Self {
        Params::Int8Array(a)
    }
}

impl<'a> From<&'a [i16]> for Params<'a> {
    fn from(a: &'a [i16]) -> Self {
        Params::Int16Array(a)
    }
}

impl<'a> From<&'a [i32]> for Params<'a> {
    fn from(a: &'a [i32]) -> Self {
        Params::Int32Array(a)
    }
}

impl<'a> From<&'a [i64]> for Params<'a> {
    fn from(a: &'a [i64]) -> Self {
        Params::Int64Array(a)
    }
}

impl<'a> From<&'a [u8]> for Params<'a> {
    fn from(a: &'a [u8]) -> Self {
        Params::Uint8Array(a)
    }
}

impl<'a> From<&'a [u16]> for Params<'a> {
    fn from(a: &'a [u16]) -> Self {
        Params::Uint16Array(a)
    }
}

impl<'a> From<&'a [u32]> for Params<'a> {
    fn from(a: &'a [u32]) -> Self {
        Params::Uint32Array(a)
    }
}

impl<'a> From<&'a [u64]> for Params<'a> {
    fn from(a: &'a [u64]) -> Self {
        Params::Uint64Array(a)
    }
}

/// [`StrLocation`] represent the underlying location of an
/// encoded string which points to the relevant starting index
/// and length from that index location, this then can be
/// applied to any valid memory address that contains the texts
/// to find the relevant portion.
pub struct StrLocation(u64, u64);

#[allow(clippy::len_without_is_empty)]
impl StrLocation {
    pub fn new(index: u64, length: u64) -> Self {
        Self(index, length)
    }

    pub fn index(&self) -> u64 {
        self.0
    }

    pub fn len(&self) -> u64 {
        self.1
    }
}

/// [`ArgumentOperations`] representing the argument layout
/// in memory used to represent the different argument blocks
/// a function is to receive.
///
/// It must always start with a `Start` and end with a `Stop`.
/// In Between each argument is wrapped by a `Begin` and a `End`
/// as many arguments are necessary for encoding.
///
/// So layout: [Start, [Begin, End]**, Stop]
///
///
/// In Actual Layout:
///
/// Memory Layout: [
///     1 Byte / 8 Bits for Start,
///     1 Byte / 8 Bits for Begin,
///     4 Bytes / 16 bits for Size of content
///     [CONTENT]
///     1 Byte / 8 Bits for End,
///     1 Byte / 8 Bits for Stop,
/// ]
///
/// All together its: 21 Bytes = 168 bits Long.
///
/// Adding the Begin (1 Byte) and Stop (1 Byte) bytes then we have additional 2 bytes = 16 bits
///
/// So in total we will have 23 Bytes = 184 bits long.
///
///
#[repr(usize)]
pub enum ArgumentOperations {
    Start = 1,
    Begin = 2,
    End = 3,
    Stop = 4,
}

#[allow(clippy::from_over_into)]
impl Into<u8> for ArgumentOperations {
    fn into(self) -> u8 {
        self as u8
    }
}

/// Lists all possible encodable operations supported
/// by this crate. This allows us represent different
/// actions and operations as with a 1 byte pointer
/// in the range of 0 - 277.
///
/// The expectation is that all operations will follow the
/// underlying layout:
///
/// Op: [Begin, Operation, [Operation Data], Stop]
///
/// Where memory will have multiple of these layout in linear
/// memory:
///
/// Memory: [Op, Op, ....]
///
#[repr(usize)]
#[derive(Clone)]
pub enum Operations {
    /// Begin - Indicative of the start of a operation in a batch, generally
    /// you should only ever see this once until the batch ends with a [`Operations::Stop`].
    /// After the begin is seen, you should see other operations indicative of what the
    /// sub-operation in the batch represent and it's specific layout.
    ///
    /// Memory wise: This is 1 Byte = 8 bits.
    ///
    Begin = 0,

    /// MakeFunction represents the operation to create/register
    /// a function on the other side at a specific ExternalReference
    /// usually pre-allocated via some API call.
    ///
    /// The layout will have the function address followed by the
    /// binary representation of the location of the function
    /// definition in batch memory, the starting index of the
    /// string and the length of the string bytes.
    ///
    /// Layout: [1, [MemoryAllocationAddress], PreAllocatedExternalReference, StartIndex, Length]
    ///
    /// In Actual Layout:
    ///
    /// Memory Layout: [
    ///     1 Byte / 8 Bits for Operations type,
    ///     4 Bytes for Memory Address for Location,
    ///     8 Bytes for External Reference that is 64bit long,
    ///     4 Bytes for Start Index,
    ///     4 bytes for Length,
    /// ]
    ///
    /// All together its: 21 Bytes = 168 bits Long.
    ///
    /// Adding the Begin (1 Byte) and Stop (1 Byte) bytes then we have additional 2 bytes = 16 bits
    ///
    /// So in total we will have 23 Bytes = 184 bits long.
    ///
    ///
    MakeFunction = 1,

    /// InvokeNoReturnFunction represents the desire to call a
    /// function across boundary that does not return any value
    /// in response to being called.
    ///
    /// It has two layout formats:
    ///
    /// A. with no argument:
    ///
    ///     [Begin, 3, FunctionHandle(u64), End]
    ///
    /// B. with arguments
    ///
    ///     [Begin, 3, FunctionHandle(u64), FunctionArguments, [Arguments], End]
    InvokeNoReturnFunction = 2,

    /// InvokeReturningFunction represents the desire to call a
    /// function across boundary that returns a value of
    /// defined type matching [`ReturnType`]
    /// in response to being called.
    ///
    /// It has two layout formats:
    ///
    /// A. with no argument:
    ///
    ///     [Begin, 3, FunctionHandle(u64), ReturnType, End]
    ///
    /// B. with arguments
    ///
    ///     [Begin, 3, FunctionHandle(u64), ReturnType, [Arguments], End]
    InvokeReturningFunction = 3,

    /// InvokeCallbackFunction represents the desire to call a
    /// function across boundary that takes a callback external reference
    /// which it will use to supply appropriate response when ready (say async call)
    /// as response to being called.
    ///
    /// It has a single layout formats:
    ///
    ///     [Begin, 3, FunctionHandle(u64), ArgStart, ArgBegin, ExternReference, ArgEnd, ArgStop, End]
    InvokeCallbackFunction = 4,

    /// Stop - indicates the end of an operation in a batch, since
    /// a memory will contain multiple operations batched into a single
    /// memory slot, until you see this 1 byte signal then you should
    /// consider that batch yet to finish.
    ///
    /// Memory wise: This is 1 Byte = 8 bits.
    ///
    Stop = 255,
}

#[allow(clippy::from_over_into)]
impl Into<u8> for Operations {
    fn into(self) -> u8 {
        self as u8
    }
}

// [`CallParams`] defines the underlying location of memory
// indicative of the starting pointer and length for which it
// relates to.
#[allow(unused)]
pub struct CallParams(pub *const u8, pub u64);

impl CallParams {
    #[must_use]
    pub fn new(addr: *const u8, length: u64) -> Self {
        Self(addr, length)
    }
}

/// [`InternalPointer`] identifies an external handle pointing
/// to a special pointer that is used to control/access
/// an external resource.
///
/// Can be an object, function or some other resource
/// that is to be used across wasm boundaries.
#[derive(Clone, Copy, PartialOrd, Ord, Hash, Eq, PartialEq, Debug)]
pub struct InternalPointer(u64);

impl From<u64> for InternalPointer {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl InternalPointer {
    pub const fn pointer(value: u64) -> Self {
        Self(value)
    }

    pub fn into_inner(self) -> u64 {
        self.0
    }

    pub fn clone_inner(&self) -> u64 {
        self.0
    }

    pub fn to_value_type(&self) -> ValueTypes {
        ValueTypes::InternalReference
    }
}

/// [`ExternalPointer`] identifies an external handle pointing
/// to a special pointer that is used to control/access
/// an external resource.
///
/// Can be an object, function or some other resource
/// that is to be used across wasm boundaries.
#[derive(Clone, Copy)]
pub struct ExternalPointer(u64);

impl From<u64> for ExternalPointer {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl ExternalPointer {
    pub const fn pointer(value: u64) -> Self {
        Self(value)
    }

    pub fn into_inner(self) -> u64 {
        self.0
    }

    pub fn clone_inner(&self) -> u64 {
        self.0
    }

    pub fn to_value_type(&self) -> ValueTypes {
        ValueTypes::ExternalReference
    }
}

/// [`JSEncoding`] defines a defining type to help indicate the
/// underlying encoding for a giving text body.
pub enum JSEncoding {
    UTF8,
    UTF16,
}

#[allow(clippy::from_over_into)]
impl Into<f32> for JSEncoding {
    fn into(self) -> f32 {
        match self {
            JSEncoding::UTF8 => 8.0,
            JSEncoding::UTF16 => 16.0,
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<u8> for JSEncoding {
    fn into(self) -> u8 {
        match self {
            JSEncoding::UTF8 => 8,
            JSEncoding::UTF16 => 16,
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<u32> for JSEncoding {
    fn into(self) -> u32 {
        match self {
            JSEncoding::UTF8 => 8,
            JSEncoding::UTF16 => 16,
        }
    }
}

impl From<u8> for JSEncoding {
    fn from(value: u8) -> Self {
        if value == 8 {
            return JSEncoding::UTF8;
        }
        if value == 16 {
            return JSEncoding::UTF16;
        }
        JSEncoding::UTF8
    }
}

impl From<u16> for JSEncoding {
    fn from(value: u16) -> Self {
        if value == 8 {
            return JSEncoding::UTF8;
        }
        if value == 16 {
            return JSEncoding::UTF16;
        }
        JSEncoding::UTF8
    }
}

impl From<u32> for JSEncoding {
    fn from(value: u32) -> Self {
        if value == 8 {
            return JSEncoding::UTF8;
        }
        if value == 16 {
            return JSEncoding::UTF16;
        }
        JSEncoding::UTF8
    }
}

impl From<f32> for JSEncoding {
    fn from(value: f32) -> Self {
        if value == 8.0 {
            return JSEncoding::UTF8;
        }
        if value == 16.0 {
            return JSEncoding::UTF16;
        }
        JSEncoding::UTF8
    }
}

impl From<f64> for JSEncoding {
    fn from(value: f64) -> Self {
        if value == 8.0 {
            return JSEncoding::UTF8;
        }
        if value == 16.0 {
            return JSEncoding::UTF16;
        }
        JSEncoding::UTF8
    }
}
