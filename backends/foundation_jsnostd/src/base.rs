/// [`ValueTypes`] represent the underlying type of value
/// being encoded into a binary stream.
#[repr(usize)]
pub enum ValueTypes {
    Null = 0,
    Undefined = 1,
    Bool = 2,
    String = 3,
    Int32 = 4,
    Int64 = 5,
    Uint32 = 6,
    Uint64 = 7,
    Float32 = 8,
    Float64 = 9,
    ExternalReference = 10,
    Uint8ArrayBuffer = 11,
    Uint16ArrayBuffer = 12,
    Uint32ArrayBuffer = 13,
    Uint64ArrayBuffer = 14,
    Int8ArrayBuffer = 15,
    Int16ArrayBuffer = 16,
    Int32ArrayBuffer = 17,
    Int64ArrayBuffer = 18,
    Float32ArrayBuffer = 19,
    Float64ArrayBuffer = 20,
}

#[allow(clippy::from_over_into)]
impl Into<u8> for ValueTypes {
    fn into(self) -> u8 {
        self as u8
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

    /// FunctionArguments represents the fact that the operation is a representation
    /// of arguments to be passed to the preceding operation representing the
    /// calling of a function.
    ///
    /// It usually either ends with a [`ArgumentOperations::Stop`] to indicate the
    /// end of all arguments.
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
    FunctionArguments = 2,

    /// CallNoReturnFunction represents the desire to call a
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
    CallNoReturnFunction = 3,

    /// CallReturningFunction represents the desire to call a
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
    CallReturningFunction = 4,

    /// CallCallbackFunction represents the desire to call a
    /// function across boundary that takes a callback external reference
    /// which it will use to supply appropriate response when ready (say async call)
    /// as response to being called.
    ///
    /// It has a single layout formats:
    ///
    ///     [Begin, 3, FunctionHandle(u64), ArgStart, ArgBegin, ExternReference, ArgEnd, ArgStop, End]
    CallCallbackFunction = 5,

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
