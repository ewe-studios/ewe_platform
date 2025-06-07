use alloc::vec::Vec;

/// [`TypedSlice`] represent the type of a slice which is sent over.
/// And helps the receiver know what exactly is represented by a slice of u8 array.
#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypedSlice {
    Int8 = 1,
    Int16 = 2,
    Int32 = 3,
    Int64 = 4,
    Uint8 = 5,
    Uint16 = 6,
    Uint32 = 7,
    Uint64 = 8,
    Float32 = 9,
    Float64 = 10,
}

#[allow(clippy::from_over_into)]
impl Into<u8> for &TypedSlice {
    fn into(self) -> u8 {
        *self as u8
    }
}

#[allow(clippy::from_over_into)]
impl Into<u8> for TypedSlice {
    fn into(self) -> u8 {
        self as u8
    }
}

impl From<u8> for TypedSlice {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Int8,
            2 => Self::Int16,
            3 => Self::Int32,
            4 => Self::Int64,
            5 => Self::Uint8,
            6 => Self::Uint16,
            7 => Self::Uint32,
            8 => Self::Uint64,
            9 => Self::Float32,
            10 => Self::Float64,
            _ => unreachable!("should not have any other type of ArgumentOperations"),
        }
    }
}

/// [`ValueTypes`] represent the underlying type of value
/// being encoded into a binary stream.
#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    Int128 = 27,
    Uint128 = 28,
    CachedText = 29,
    TypedArraySlice = 30,
}

#[allow(clippy::from_over_into)]
impl Into<u8> for &ValueTypes {
    fn into(self) -> u8 {
        *self as u8
    }
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
    Int128(i128),
    Uint8(u8),
    Uint16(u16),
    Uint32(u32),
    Uint64(u64),
    Uint128(u128),
    CachedText(u64),
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
    ExternalReference(u64),
    InternalReference(u64),
    TypedArraySlice(TypedSlice, &'a [u8]),
}

impl Params<'_> {
    pub fn to_value_type_u8(&self) -> u8 {
        self.to_value_type() as u8
    }

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
            Params::Int128(_) => ValueTypes::Int128,
            Params::Uint8(_) => ValueTypes::Uint8,
            Params::Uint16(_) => ValueTypes::Uint16,
            Params::Uint32(_) => ValueTypes::Uint32,
            Params::Uint64(_) => ValueTypes::Uint64,
            Params::Uint128(_) => ValueTypes::Uint128,
            Params::Text8(_) => ValueTypes::Text8,
            Params::Text16(_) => ValueTypes::Text16,
            Params::CachedText(_) => ValueTypes::CachedText,
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
            Params::TypedArraySlice(_, _) => ValueTypes::TypedArraySlice,
        }
    }
}

impl From<f64> for Params<'_> {
    fn from(f: f64) -> Self {
        Params::Float64(f)
    }
}

impl<'a> From<(u8, &'a [u8])> for Params<'a> {
    fn from((tp, ta): (u8, &'a [u8])) -> Self {
        Params::TypedArraySlice(tp.into(), ta)
    }
}

impl<'a> From<(TypedSlice, &'a [u8])> for Params<'a> {
    fn from((tp, ta): (TypedSlice, &'a [u8])) -> Self {
        Params::TypedArraySlice(tp, ta)
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

impl From<u128> for Params<'_> {
    fn from(i: u128) -> Self {
        Params::Uint128(i)
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

impl From<i128> for Params<'_> {
    fn from(i: i128) -> Self {
        Params::Int128(i)
    }
}

impl From<isize> for Params<'_> {
    fn from(i: isize) -> Self {
        Params::Float64(i as f64)
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
        Params::InternalReference(i.into_inner())
    }
}

impl<'a> From<&'a ExternalPointer> for Params<'a> {
    fn from(i: &'a ExternalPointer) -> Self {
        Params::ExternalReference(i.into_inner())
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
/// Memory Layout: {
///     1 Byte / 8 Bits for Start,
///     1 Byte / 8 Bits for Begin,
///     4 Bytes / 16 bits for Size of content
///     1 Byte / 8 bytes for Type Optimization (Default: None, value = 0)
///     [CONTENT]
///     1 Byte / 8 Bits for End,
///     1 Byte / 8 Bits for Stop,
/// }
///
/// All together its: 22 Bytes = 176 bits Long.
///
/// Adding the Begin (1 Byte) and Stop (1 Byte) bytes then we have additional 2 bytes = 16 bits
///
/// So in total we will have 24 Bytes = 192 bits long for the Arguments section.
///
/// Note because of the [`TypeOptimization`] byte indicator the [CONTENT] might be shorter
/// than it's actual type.
///
///
#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgumentOperations {
    Start = 1,
    Begin = 2,
    End = 3,
    Stop = 4,
}

#[allow(clippy::from_over_into)]
impl Into<u8> for &ArgumentOperations {
    fn into(self) -> u8 {
        *self as u8
    }
}

#[allow(clippy::from_over_into)]
impl Into<u8> for ArgumentOperations {
    fn into(self) -> u8 {
        self as u8
    }
}

impl From<u8> for ArgumentOperations {
    fn from(value: u8) -> Self {
        match value {
            1 => ArgumentOperations::Start,
            2 => ArgumentOperations::Begin,
            3 => ArgumentOperations::End,
            4 => ArgumentOperations::Stop,
            _ => unreachable!("should not have any other type of ArgumentOperations"),
        }
    }
}

/// Lists all possible encodable operations supported
/// by this crate. This allows us represent different
/// actions and operations with a 1 byte pointer
/// in the range of 0 - 255 (a u8, 8 bits).
///
/// The idea is a batch of different operations is represented
/// as a memory slot sent to the other side (Host side) with
/// the precondition that a batch must start with the [Begin] byte (u8)
/// and end with a [Stop] byte (u8).
///
/// It must then follow the layout:
///
/// Op: [Begin, [Operation]*, Stop]
///
/// Where Operation: is a layout of a specific type of operation
/// with it's own underlying layout defining its components.
///
/// Operation: [Op, [OperationComponent]*, OpEnd]
///
#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    /// Layout: {1{ [MemoryAllocationAddress}, PreAllocatedExternalReference, StartIndex, Length}
    ///
    /// In Actual Layout:
    ///
    /// Memory Layout: {
    ///     1 Byte / 8 Bits for Operations type,
    ///     4 Bytes for Memory Address for Location,
    ///     8 Bytes for External Reference that is 64bit long,
    ///     4 Bytes for Start Index,
    ///     4 bytes for Length,
    /// }
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
    /// A. with no argument: Begin, 3, FunctionHandle(u64), End
    ///
    /// B. with arguments: Begin, 3, FunctionHandle(u64), FunctionArguments, {Arguments}, End
    InvokeNoReturnFunction = 2,

    /// InvokeReturningFunction represents the desire to call a
    /// function across boundary that returns a value of
    /// defined type matching [`ReturnType`]
    /// in response to being called.
    ///
    /// It has two layout formats:
    ///
    /// A. with no argument: Begin, 3, FunctionHandle(u64), ReturnType, End
    ///
    /// B. with arguments: Begin, 3, FunctionHandle(u64), ReturnType, Arguments*, End
    InvokeReturningFunction = 3,

    /// InvokeCallbackFunction represents the desire to call a
    /// function across boundary that takes a callback external reference
    /// which it will use to supply appropriate response when ready (say async call)
    /// as response to being called.
    ///
    /// Layout format: Begin, 3, FunctionHandle(u64), ArgStart, ArgBegin, ExternReference, ArgEnd, ArgStop,
    ///  End
    InvokeCallbackFunction = 4,

    /// End - indicates the end of a portion of a instruction set.
    /// Since an instruction memory array can contain multiple instructions
    /// batched together, then each instruction must have a end marker indicating
    /// one portion is over.
    End = 254,

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
impl Into<u8> for &Operations {
    fn into(self) -> u8 {
        *self as u8
    }
}

#[allow(clippy::from_over_into)]
impl Into<u8> for Operations {
    fn into(self) -> u8 {
        self as u8
    }
}

impl From<u8> for Operations {
    fn from(value: u8) -> Self {
        match value {
            0 => Operations::Begin,
            1 => Operations::MakeFunction,
            2 => Operations::InvokeNoReturnFunction,
            3 => Operations::InvokeReturningFunction,
            4 => Operations::InvokeCallbackFunction,
            254 => Operations::End,
            255 => Operations::Stop,
            _ => unreachable!("should not have any other type of ArgumentOperations"),
        }
    }
}

// [`CallParams`] defines the underlying location of memory
// indicative of the starting pointer and length for which it
// relates to.
#[allow(unused)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallParams(pub *const u8, pub u64);

impl CallParams {
    #[must_use]
    pub fn new(addr: *const u8, length: u64) -> Self {
        Self(addr, length)
    }
}

/// [`CachedText8`] defines a instance of a cached UTF-8 text at some
/// specific location managed by the host runtime identified by the
/// wrapped u64 id.
pub struct CachedText(u64);

impl From<u64> for CachedText {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl CachedText {
    pub const fn pointer(value: u64) -> Self {
        Self(value)
    }

    pub fn into_param<'a>(self) -> Params<'a> {
        Params::CachedText(self.0)
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// [`TypeOptimization`] represent potential type optimization that can happen to types
/// represented as a single [`u8`] (max of 255) numbers. This allows us
/// declare within the format any potential optimization and space saving
/// operation that might have occurred for a giving type, informing
/// the HOST side about so it can correctly decode the underlying content.
#[repr(usize)]
#[derive(Debug, PartialEq, PartialOrd, Eq, Hash, Clone, Copy)]
pub enum TypeOptimization {
    None = 0,

    // optimize ints
    QuantizedInt16AsI8 = 1,
    QuantizedInt32AsI8 = 2,
    QuantizedInt32AsI16 = 3,
    QuantizedInt64AsI8 = 4,
    QuantizedInt64AsI16 = 5,
    QuantizedInt64AsI32 = 6,

    // optimize uints
    QuantizedUint16AsU8 = 7,
    QuantizedUint32AsU8 = 8,
    QuantizedUint32AsU16 = 9,
    QuantizedUint64AsU8 = 10,
    QuantizedUint64AsU16 = 11,
    QuantizedUint64AsU32 = 12,

    // optimize floats
    QuantizedF64AsF32 = 13,

    // TODO(alex.ewetumo): Add quantization for these when f128 is stable.
    //
    // these wont be supported yet as f128 is still nightly only
    // but added here for coverage
    QuantizedF128AsF32 = 14,
    QuantizedF128AsF64 = 15,

    // optimize i128 bits
    QuantizedInt128AsI8 = 16,
    QuantizedInt128AsI16 = 17,
    QuantizedInt128AsI32 = 18,
    QuantizedInt128AsI64 = 19,

    // optimize u128 bits
    QuantizedUint128AsU8 = 20,
    QuantizedUint128AsU16 = 21,
    QuantizedUint128AsU32 = 22,
    QuantizedUint128AsU64 = 23,

    // optimize pointers bits
    QuantizedPtrAsU8 = 24,
    QuantizedPtrAsU16 = 25,
    QuantizedPtrAsU32 = 26,
    QuantizedPtrAsU64 = 27,
}

#[allow(clippy::from_over_into)]
impl Into<u8> for TypeOptimization {
    fn into(self) -> u8 {
        self as u8
    }
}

impl core::fmt::Display for TypeOptimization {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<u8> for TypeOptimization {
    fn from(value: u8) -> Self {
        match value {
            0 => TypeOptimization::None,
            1 => TypeOptimization::QuantizedInt16AsI8,
            2 => TypeOptimization::QuantizedInt32AsI8,
            3 => TypeOptimization::QuantizedInt32AsI16,
            4 => TypeOptimization::QuantizedInt64AsI8,
            5 => TypeOptimization::QuantizedInt64AsI16,
            6 => TypeOptimization::QuantizedInt64AsI32,
            7 => TypeOptimization::QuantizedUint16AsU8,
            8 => TypeOptimization::QuantizedUint32AsU8,
            9 => TypeOptimization::QuantizedUint32AsU16,
            10 => TypeOptimization::QuantizedUint64AsU8,
            11 => TypeOptimization::QuantizedUint64AsU16,
            12 => TypeOptimization::QuantizedUint64AsU32,
            13 => TypeOptimization::QuantizedF64AsF32,
            14 => TypeOptimization::QuantizedF128AsF32,
            15 => TypeOptimization::QuantizedF128AsF64,
            16 => TypeOptimization::QuantizedInt128AsI8,
            17 => TypeOptimization::QuantizedInt128AsI16,
            18 => TypeOptimization::QuantizedInt128AsI32,
            19 => TypeOptimization::QuantizedInt128AsI64,
            20 => TypeOptimization::QuantizedUint128AsU8,
            21 => TypeOptimization::QuantizedUint128AsU16,
            22 => TypeOptimization::QuantizedUint128AsU32,
            23 => TypeOptimization::QuantizedUint128AsU64,
            24 => TypeOptimization::QuantizedPtrAsU8,
            25 => TypeOptimization::QuantizedPtrAsU16,
            26 => TypeOptimization::QuantizedPtrAsU32,
            27 => TypeOptimization::QuantizedPtrAsU64,
            _ => unreachable!("should not have any other type of ArgumentOperations"),
        }
    }
}

#[derive(Debug)]
pub enum MemOpError {
    FailedQuantization,
}

pub type MemOpResult<T> = core::result::Result<T, MemOpError>;

impl core::error::Error for MemOpError {}

impl core::fmt::Display for MemOpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// [`value_quantitzation`] contains various quantitzation methods
/// which exists to take different unit value types returning a more
/// compact representation of those values with a lesser unit type where
/// possible.
///
/// Most of it is derived from the following sampkle code taking from
/// a rust playground which experiments with quantizing pointers to values.
///
/// ```rust
/// use std::println;
///
///  let n: u128 = 1;
///  let n_u8_pointer = (n as *const u8) as u128;
///
///  match n_u8_pointer {
///      0..=255 => {
///          let as_bit = n_u8_pointer as u8;
///          let as_bit_bytes = as_bit.to_le_bytes();
///
///          println!("ValueAs8: {:?} -> {:?}", as_bit, as_bit_bytes);
///      }
///      256..=65535 => {
///          let as_bit = n_u8_pointer as u16;
///          let as_bit_bytes = as_bit.to_le_bytes();
///
///          println!("ValueAs16: {:?} -> {:?}", as_bit, as_bit_bytes);
///      }
///      65536..=4294967295 => {
///          let as_bit = n_u8_pointer as u32;
///          let as_bit_bytes = as_bit.to_le_bytes();
///
///          println!("ValueAs32: {:?} -> {:?}", as_bit, as_bit_bytes);
///      }
///      4294967296..=18446744073709551615 => {
///          let as_bit = n_u8_pointer as u64;
///          let as_bit_bytes = as_bit.to_le_bytes();
///
///          println!("ValueAs64: {:?} -> {:?}", as_bit, as_bit_bytes);
///      }
///      _ => {
///          let as_bit_bytes = n_u8_pointer.to_le_bytes();
///          println!("ValueAsU64: {:?} -> {:?}", n_u8_pointer, as_bit_bytes);
///      }
///  };
/// ```
///
pub mod value_quantitization {
    use super::*;

    /// [`qi16`] performs an operation to transform
    /// a [`i16`] large number into bytes with an
    /// optimization that if the giving number
    /// is within the ranges of the lower number types it will
    /// first convert the number into that type then return
    /// the binary in little endian and the [`TypeOptimization`]
    /// applied to the value.
    pub fn qi16(value: i16) -> (Vec<u8>, TypeOptimization) {
        match value {
            -128..=127 => {
                let as_bit = value as i8;
                let as_bit_bytes = as_bit.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::QuantizedInt16AsI8)
            }
            _ => {
                let as_bit_bytes = value.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::None)
            }
        }
    }

    /// [`qi32`] performs an operation to transform
    /// a [`i32`] large number (applies for i16 up to i32)
    /// into bytes with an optimization that if the giving number
    /// is within the ranges of the lower number types it will
    /// first convert the number into that type then return
    /// the binary in little endian and the [`TypeOptimization`]
    /// applied to the value.
    pub fn qi32(value: i32) -> (Vec<u8>, TypeOptimization) {
        match value {
            -128..=127 => {
                let as_bit = value as i8;
                let as_bit_bytes = as_bit.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::QuantizedInt32AsI8)
            }
            -32768..=-129 | 128..=32767 => {
                let as_bit = value as i16;
                let as_bit_bytes = as_bit.to_le_bytes();

                (as_bit_bytes.to_vec(), TypeOptimization::QuantizedInt32AsI16)
            }
            _ => {
                let as_bit_bytes = value.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::None)
            }
        }
    }

    /// [`qi64`] performs an operation to transform
    /// a [`i64`] large number (applies for i16 up to i64)
    /// into bytes with an optimization that if the giving number
    /// is within the ranges of the lower number types it will
    /// first convert the number into that type then return
    /// the binary in little endian and the [`TypeOptimization`]
    /// applied to the value.
    pub fn qi64(value: i64) -> (Vec<u8>, TypeOptimization) {
        match value {
            -128..=127 => {
                let as_bit = value as i8;
                let as_bit_bytes = as_bit.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::QuantizedInt64AsI8)
            }
            -32768..=-129 | 128..=32767 => {
                let as_bit = value as i16;
                let as_bit_bytes = as_bit.to_le_bytes();

                (as_bit_bytes.to_vec(), TypeOptimization::QuantizedInt64AsI16)
            }
            -2147483648..=-32769 | 32768..=2147483647 => {
                let as_bit = value as i32;
                let as_bit_bytes = as_bit.to_le_bytes();

                (as_bit_bytes.to_vec(), TypeOptimization::QuantizedInt64AsI32)
            }
            _ => {
                let as_bit_bytes = value.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::None)
            }
        }
    }

    /// [`qi128`] performs an operation to transform
    /// a [`i128`] large number (applies for i16 up to i128)
    /// into bytes with an optimization that if the giving number
    /// is within the ranges of the lower number types it will
    /// first convert the number into that type then return
    /// the binary in little endian and the [`TypeOptimization`]
    /// applied to the value.
    pub fn qi128(value: i128) -> (Vec<u8>, TypeOptimization) {
        match value {
            -128..=127 => {
                let as_bit = value as i8;
                let as_bit_bytes = as_bit.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::QuantizedInt128AsI8)
            }
            -32768..=-129 | 128..=32767 => {
                let as_bit = value as i16;
                let as_bit_bytes = as_bit.to_le_bytes();

                (
                    as_bit_bytes.to_vec(),
                    TypeOptimization::QuantizedInt128AsI16,
                )
            }
            -2147483648..=-32769 | 32768..=2147483647 => {
                let as_bit = value as i32;
                let as_bit_bytes = as_bit.to_le_bytes();

                (
                    as_bit_bytes.to_vec(),
                    TypeOptimization::QuantizedInt128AsI32,
                )
            }
            -9223372036854775808..=-2147483649 | 2147483648..=9223372036854775807 => {
                let as_bit = value as i64;
                let as_bit_bytes = as_bit.to_le_bytes();

                (
                    as_bit_bytes.to_vec(),
                    TypeOptimization::QuantizedInt128AsI64,
                )
            }
            _ => {
                // get the MSB by shifting right 64 bits
                let value_msb: i64 = (value >> 64) as i64;

                // get the LSB by truncating to i64
                let value_lsb: i64 = value as i64;

                let msb_bytes = value_msb.to_le_bytes();
                let lsb_bytes = value_lsb.to_le_bytes();

                let mut content = Vec::with_capacity(msb_bytes.len() + lsb_bytes.len());
                content.extend_from_slice(&msb_bytes);
                content.extend_from_slice(&lsb_bytes);

                (content, TypeOptimization::None)
            }
        }
    }

    /// [`qu16`] performs an operation to transform
    /// a [`u16`] large number into bytes with an
    /// optimization that if the giving number
    /// is within the ranges of the lower number types it will
    /// first convert the number into that type then return
    /// the binary in little endian and the [`TypeOptimization`]
    /// applied to the value.
    pub fn qu16(value: u16) -> (Vec<u8>, TypeOptimization) {
        match value {
            0..=255 => {
                let as_bit = value as u8;
                let as_bit_bytes = as_bit.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::QuantizedUint16AsU8)
            }
            _ => {
                let as_bit_bytes = value.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::None)
            }
        }
    }

    /// [`qu32`] performs an operation to transform
    /// a [`u32`] large number into bytes with an
    /// optimization that if the giving number
    /// is within the ranges of the lower number types it will
    /// first convert the number into that type then return
    /// the binary in little endian and the [`TypeOptimization`]
    /// applied to the value.
    pub fn qu32(value: u32) -> (Vec<u8>, TypeOptimization) {
        match value {
            0..=255 => {
                let as_bit = value as u8;
                let as_bit_bytes = as_bit.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::QuantizedUint32AsU8)
            }
            256..=65535 => {
                let as_bit = value as u16;
                let as_bit_bytes = as_bit.to_le_bytes();

                (
                    as_bit_bytes.to_vec(),
                    TypeOptimization::QuantizedUint32AsU16,
                )
            }
            _ => {
                let as_bit_bytes = value.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::None)
            }
        }
    }

    // [`qf64`] quantizes a f64 into a f32 if its within the range there by reducing
    // the actual bytes needed to communicate it from 8 to 4 and when you have
    // alot of these, this will save us alot of space.
    //
    // When its actuall in the f64 range then the normal byte count is used with quantization
    // set as [`TypeOptimization::None`].
    pub fn qf64(value: f64) -> (Vec<u8>, TypeOptimization) {
        const F32_MIN: f64 = f32::MIN as f64;
        const F32_MAX: f64 = f32::MAX as f64;

        match value {
            F32_MIN..=F32_MAX => {
                let as_bit = value as f32;
                let as_bit_bytes = as_bit.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::QuantizedF64AsF32)
            }
            _ => {
                let as_bit_bytes = value.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::None)
            }
        }
    }

    /// [`qu64`] performs an operation to transform
    /// a [`u64`] large number (applies for u16 up to u64)
    /// into bytes with an optimization that if the giving number
    /// is within the ranges of the lower number types it will
    /// first convert the number into that type then return
    /// the binary in little endian and the [`TypeOptimization`]
    /// applied to the value.
    pub fn qu64(value: u64) -> (Vec<u8>, TypeOptimization) {
        match value {
            0..=255 => {
                let as_bit = value as u8;
                let as_bit_bytes = as_bit.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::QuantizedUint64AsU8)
            }
            256..=65535 => {
                let as_bit = value as u16;
                let as_bit_bytes = as_bit.to_le_bytes();

                (
                    as_bit_bytes.to_vec(),
                    TypeOptimization::QuantizedUint64AsU16,
                )
            }
            65536..=4294967295 => {
                let as_bit = value as u32;
                let as_bit_bytes = as_bit.to_le_bytes();

                (
                    as_bit_bytes.to_vec(),
                    TypeOptimization::QuantizedUint64AsU32,
                )
            }
            _ => {
                let as_bit_bytes = value.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::None)
            }
        }
    }

    /// [`qu128`] performs an operation to transform
    /// a [`u128`] large number (applies for u16 up to u128)
    /// into bytes with an optimization that if the giving number
    /// is within the ranges of the lower number types it will
    /// first convert the number into that type then return
    /// the binary in little endian and the [`TypeOptimization`]
    /// applied to the value.
    pub fn qu128(value: u128) -> (Vec<u8>, TypeOptimization) {
        match value {
            0..=255 => {
                let as_bit = value as u8;
                let as_bit_bytes = as_bit.to_le_bytes();
                (
                    as_bit_bytes.to_vec(),
                    TypeOptimization::QuantizedUint128AsU8,
                )
            }
            256..=65535 => {
                let as_bit = value as u16;
                let as_bit_bytes = as_bit.to_le_bytes();

                (
                    as_bit_bytes.to_vec(),
                    TypeOptimization::QuantizedUint128AsU16,
                )
            }
            65536..=4294967295 => {
                let as_bit = value as u32;
                let as_bit_bytes = as_bit.to_le_bytes();

                (
                    as_bit_bytes.to_vec(),
                    TypeOptimization::QuantizedUint128AsU32,
                )
            }
            4294967296..=18446744073709551615 => {
                let as_bit = value as u64;
                let as_bit_bytes = as_bit.to_le_bytes();

                (
                    as_bit_bytes.to_vec(),
                    TypeOptimization::QuantizedUint128AsU64,
                )
            }
            _ => {
                // nice trick to switch all bits to 1 for a 64bit number.
                const MASK: u128 = 0xFFFFFFFFFFFFFFFF;

                // get the MSB by shifting right 64 bits
                let value_msb = (value >> 64) as u64;

                // get the LSB by truncating to u64, this gets automatically truncated
                // but we also just use u64::MAX to mask it to the lowest 64 bits
                // or LSB.
                let value_lsb = (value & MASK) as u64;

                // You can always recombine them in this way:
                //
                // let value_back = (value_msb as u128) << 64;
                // let value_front = value_lsb as u128;
                // let value_up = value_back | value_front;

                let msb_bytes = value_msb.to_le_bytes();
                let lsb_bytes = value_lsb.to_le_bytes();

                let mut content = Vec::with_capacity(msb_bytes.len() + lsb_bytes.len());
                content.extend_from_slice(&msb_bytes);
                content.extend_from_slice(&lsb_bytes);

                (content, TypeOptimization::None)
            }
        }
    }

    /// [`qpointer`] attempts to quantize a pointer value expressed as either
    /// a u8, u16, u32 or u64 depending on the range the pointer value falls under.
    pub fn qpointer(ptr: *const u8) -> (Vec<u8>, TypeOptimization) {
        match ptr as u64 {
            0..=255 => {
                let as_bit = ptr as u8;
                let as_bit_bytes = as_bit.to_le_bytes();
                (as_bit_bytes.to_vec(), TypeOptimization::QuantizedPtrAsU8)
            }
            256..=65535 => {
                let as_bit = ptr as u16;
                let as_bit_bytes = as_bit.to_le_bytes();

                (as_bit_bytes.to_vec(), TypeOptimization::QuantizedPtrAsU16)
            }
            65536..=4294967295 => {
                let as_bit = ptr as u32;
                let as_bit_bytes = as_bit.to_le_bytes();

                (as_bit_bytes.to_vec(), TypeOptimization::QuantizedPtrAsU32)
            }
            // By default we already expecting a U64 pointer (maybe one-day it might become a u129)
            // 4294967296..=18446744073709551615 => {
            //     let ptr_as_u64 = ptr as u64;
            //     let as_bit_bytes = ptr_as_u64.to_le_bytes();
            //
            //     (as_bit_bytes.to_vec(), TypeOptimization::QuantizedPtrAsU64)
            // }
            _ => {
                let ptr_as_u64 = ptr as u64;
                let as_bit_bytes = ptr_as_u64.to_le_bytes();

                (as_bit_bytes.to_vec(), TypeOptimization::None)
            }
        }
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

/// [`BIT_SIZE`] represent the shifting we want to do
/// to shift 32 bit numbers into 64bit numbers.
const BIT_SIZE: u64 = 32;

/// [`BIT_MASK`] representing the needing masking
/// to be used in bitpacking two 32bit numbers into
/// a 64 bit number.
const BIT_MASK: u64 = 0xFFFFFFFF;

/// [`MemoryId`] represents a key to a allocation '
/// which has a unique generation to denote it's ownership
/// if the generation differs from the current generation of
/// a given index then that means ownership was already lost and
/// hence cant be used.
///
/// First Elem - is the index
/// Second Elem - is the generation
///
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct MemoryId(pub(crate) u32, pub(crate) u32);

impl From<u64> for MemoryId {
    fn from(value: u64) -> Self {
        MemoryId::from_u64(value)
    }
}

#[allow(clippy::from_over_into)]
impl Into<u64> for MemoryId {
    fn into(self) -> u64 {
        self.as_u64()
    }
}

impl MemoryId {
    /// [`from_u64`] implements conversion of a 64bit unsighed int
    /// into a Memory by the assuming that the First 32bit represent
    /// the index (LSB) and the last 32 bit (MSB) represent the
    /// generation number.
    pub fn from_u64(memory_id: u64) -> Self {
        let index = ((memory_id >> BIT_SIZE) & BIT_MASK) as u32; // upper bit
        let generation = (memory_id & BIT_MASK) as u32; // lower bit
        Self(index, generation)
    }

    /// [`as_u64`] packs the index and generation represented
    /// by the [`MemoryId`] into a singular u64 number allowing
    /// memory savings and improved cross over sharing.
    pub fn as_u64(&self) -> u64 {
        let msb_bit = ((self.0 as u64) & BIT_MASK) << BIT_SIZE; // Upper 32 bits at the MSB
        let lsb_bit = (self.1 as u64) & BIT_MASK; // Lower 32 bits at the LSB
        msb_bit | lsb_bit
    }

    pub fn index(&self) -> u32 {
        self.0
    }

    pub fn generation(&self) -> u32 {
        self.1
    }
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Debug)]
pub struct CompletedInstructions {
    pub ops_id: MemoryId,
    pub text_id: MemoryId,
}

#[cfg(test)]
mod quantization_tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn can_quantize_ptr() {
        struct TestCase {
            value: *const u8,
            expected_bytes: Vec<u8>,
            quantization: TypeOptimization,
        }

        let test_cases: Vec<TestCase> = vec![
            TestCase {
                value: 20 as *const u8,
                expected_bytes: vec![20],
                quantization: TypeOptimization::QuantizedPtrAsU8,
            },
            TestCase {
                value: 32767 as *const u8,
                expected_bytes: vec![255, 127],
                quantization: TypeOptimization::QuantizedPtrAsU16,
            },
            TestCase {
                value: 2147483647 as *const u8,
                expected_bytes: vec![255, 255, 255, 127],
                quantization: TypeOptimization::QuantizedPtrAsU32,
            },
            TestCase {
                value: 6294967296 as *const u8,
                expected_bytes: vec![0, 148, 53, 119, 1, 0, 0, 0],
                quantization: TypeOptimization::None,
            },
            TestCase {
                value: 9223372036854775809 as *const u8,
                expected_bytes: vec![1, 0, 0, 0, 0, 0, 0, 128],
                quantization: TypeOptimization::None,
            },
        ];

        for test_case in test_cases {
            let (content, tq) = value_quantitization::qpointer(test_case.value);
            assert_eq!(
                test_case.expected_bytes, content,
                "Output bytes should match"
            );
            assert_eq!(test_case.quantization, tq, "Quantization type should match");
        }
    }

    #[test]
    fn can_quantize_i128() {
        struct TestCase {
            value: i128,
            expected_bytes: Vec<u8>,
            quantization: TypeOptimization,
        }

        let test_cases: Vec<TestCase> = vec![
            TestCase {
                value: 20,
                expected_bytes: vec![20],
                quantization: TypeOptimization::QuantizedInt128AsI8,
            },
            TestCase {
                value: 32767,
                expected_bytes: vec![255, 127],
                quantization: TypeOptimization::QuantizedInt128AsI16,
            },
            TestCase {
                value: 2147483647,
                expected_bytes: vec![255, 255, 255, 127],
                quantization: TypeOptimization::QuantizedInt128AsI32,
            },
            TestCase {
                value: 6294967296,
                expected_bytes: vec![0, 148, 53, 119, 1, 0, 0, 0],
                quantization: TypeOptimization::QuantizedInt128AsI64,
            },
            TestCase {
                value: 9223372036854775809,
                expected_bytes: vec![0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 128],
                quantization: TypeOptimization::None,
            },
        ];

        for test_case in test_cases {
            let (content, tq) = value_quantitization::qi128(test_case.value);
            assert_eq!(
                test_case.expected_bytes, content,
                "Output bytes should match"
            );
            assert_eq!(test_case.quantization, tq, "Quantization type should match");
        }
    }

    #[test]
    fn can_quantize_u128() {
        struct TestCase {
            value: u128,
            expected_bytes: Vec<u8>,
            quantization: TypeOptimization,
        }

        let test_cases: Vec<TestCase> = vec![
            TestCase {
                value: 20,
                expected_bytes: vec![20],
                quantization: TypeOptimization::QuantizedUint128AsU8,
            },
            TestCase {
                value: 65535,
                expected_bytes: vec![255, 255],
                quantization: TypeOptimization::QuantizedUint128AsU16,
            },
            TestCase {
                value: 4294967295,
                expected_bytes: vec![255, 255, 255, 255],
                quantization: TypeOptimization::QuantizedUint128AsU32,
            },
            TestCase {
                value: 6294967296,
                expected_bytes: vec![0, 148, 53, 119, 1, 0, 0, 0],
                quantization: TypeOptimization::QuantizedUint128AsU64,
            },
            TestCase {
                value: 18446744073709551619,
                expected_bytes: vec![1, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0],
                quantization: TypeOptimization::None,
            },
        ];

        for test_case in test_cases {
            let (content, tq) = value_quantitization::qu128(test_case.value);
            assert_eq!(
                test_case.expected_bytes, content,
                "Output bytes should match"
            );
            assert_eq!(test_case.quantization, tq, "Quantization type should match");
        }
    }

    #[test]
    fn can_quantize_u64() {
        struct TestCase {
            value: u64,
            expected_bytes: Vec<u8>,
            quantization: TypeOptimization,
        }

        let test_cases: Vec<TestCase> = vec![
            TestCase {
                value: 20,
                expected_bytes: vec![20],
                quantization: TypeOptimization::QuantizedUint64AsU8,
            },
            TestCase {
                value: 65535,
                expected_bytes: vec![255, 255],
                quantization: TypeOptimization::QuantizedUint64AsU16,
            },
            TestCase {
                value: 4294967295,
                expected_bytes: vec![255, 255, 255, 255],
                quantization: TypeOptimization::QuantizedUint64AsU32,
            },
            TestCase {
                value: 6294967296,
                expected_bytes: vec![0, 148, 53, 119, 1, 0, 0, 0],
                quantization: TypeOptimization::None,
            },
            TestCase {
                value: 4294967296,
                expected_bytes: vec![0, 0, 0, 0, 1, 0, 0, 0],
                quantization: TypeOptimization::None,
            },
        ];

        for test_case in test_cases {
            let (content, tq) = value_quantitization::qu64(test_case.value);
            assert_eq!(
                test_case.expected_bytes, content,
                "Output bytes should match"
            );
            assert_eq!(test_case.quantization, tq, "Quantization type should match");
        }
    }

    #[test]
    fn can_quantize_i64() {
        struct TestCase {
            value: i64,
            expected_bytes: Vec<u8>,
            quantization: TypeOptimization,
        }

        let test_cases: Vec<TestCase> = vec![
            TestCase {
                value: 20,
                expected_bytes: vec![20],
                quantization: TypeOptimization::QuantizedInt64AsI8,
            },
            TestCase {
                value: 32767,
                expected_bytes: vec![255, 127],
                quantization: TypeOptimization::QuantizedInt64AsI16,
            },
            TestCase {
                value: 2147483647,
                expected_bytes: vec![255, 255, 255, 127],
                quantization: TypeOptimization::QuantizedInt64AsI32,
            },
            TestCase {
                value: 6294967296,
                expected_bytes: vec![0, 148, 53, 119, 1, 0, 0, 0],
                quantization: TypeOptimization::None,
            },
        ];

        for test_case in test_cases {
            let (content, tq) = value_quantitization::qi64(test_case.value);
            assert_eq!(
                test_case.expected_bytes, content,
                "Output bytes should match"
            );
            assert_eq!(test_case.quantization, tq, "Quantization type should match");
        }
    }
}
