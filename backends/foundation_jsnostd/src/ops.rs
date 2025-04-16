#![allow(dead_code)]
#![allow(clippy::items_after_test_module)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]

use alloc::rc::Rc;
use alloc::string::{FromUtf16Error, FromUtf8Error, String};
use alloc::vec::Vec;
use foundation_nostd::spin::Mutex;

/// OpIndex defines the different operations that is
/// representable and performable through the underlying
/// index returned by the op_index method which returns
/// the first byte indicative of the type of operation in a
/// 32 bit number.
///
/// This allows us squeeze varying facts about a desired
/// operation to be performed into a single u64 (32 bits)
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
pub struct CallParams(pub *const u8, pub u64);

impl CallParams {
    #[must_use]
    pub fn new(addr: *const u8, length: u64) -> Self {
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

    fn borrow(&self) -> &u64 {
        &self.0
    }

    fn clone_inner(&self) -> u64 {
        self.0
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
/// context that this is used to represent the location
/// of a runtime function across WASM boundary.
pub type FuncHandle = u64;

/// FuncCall defines the different function calls
/// we can possible make which should be supported
/// by whatever underlying runtime environment
/// gets the underlying binary representation
/// without any form of deserialization efforts
/// as this will effectively be represented in binary
pub enum FuncCall {
    No(FuncHandle, CallParams),
    I64(FuncHandle, CallParams),
    I32(FuncHandle, CallParams),
    U64(FuncHandle, CallParams),
    String(FuncHandle, CallParams),
    Object(FuncHandle, CallParams),
}

define_index!(1, FuncCall);

#[cfg(test)]
mod test_func_call {
    use crate::ops::{CallParams, FuncCall, OpIndex};

    #[test]
    fn can_get_func_call() {
        let handler = FuncCall::No(0, CallParams::new(&0, 10));
        assert_eq!(handler.op_index(), 1);
    }
}

/// [`StrLocation`] represent the underlying location of an
/// encoded string which points to the relevant address
/// of the string and it's underlying starting index and
/// length from that index location.
pub struct StrLocation(pub *const u8, pub u8, pub u8);

impl StrLocation {
    /// new returns a representation pointing to a specific
    /// string within a specific memory location where a specific string
    /// slice is located within that memory location.
    ///
    /// Allows us combine multiple string within a single memory
    /// location but still be able to represent each specifically
    /// within this singular memory.
    pub fn new(pointer: *const u8, from: u8, length: u8) -> Self {
        Self(pointer, from, length)
    }
}

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

/// [`Op`] is an enum representing different operations that can be  
/// performed. It also helps avoid heap allocation by
/// attempting to use Traits to represent these different
/// operations.
pub enum Ops {
    Func(FuncCall),
}

/// A list of different Operations to be applied.
pub struct Operations(Vec<Ops>);

/// Batch represents an encoded operation that is
/// encoded as a series of ops byte each representing
/// the desired operations and a vector of bytes for the
/// potential batch string that might need to be converted
/// over the boundaries.
pub struct Batch {
    pub text: Vec<u8>,
    pub ops: Vec<u8>,
}

pub struct MemoryAllocation {
    memory: Rc<Mutex<Option<Vec<u8>>>>,
}

pub type MemoryAllocationResult<T> = core::result::Result<T, MemoryAllocationError>;

#[derive(Debug)]
pub enum MemoryAllocationError {
    NotValidUTF8(FromUtf8Error),
    NotValidUTF16(FromUtf16Error),
    NoMemoryAllocation,
    NoMoreAllocationSlots,
    InvalidAllocationId,
}

impl core::error::Error for MemoryAllocationError {}

impl core::fmt::Display for MemoryAllocationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Clone for MemoryAllocation {
    fn clone(&self) -> Self {
        Self {
            memory: self.memory.clone(),
        }
    }
}

impl MemoryAllocation {
    pub fn new(mem: Vec<u8>) -> Self {
        Self {
            memory: Rc::new(Mutex::new(Some(mem))),
        }
    }

    pub fn apply<F>(&self, f: F)
    where
        F: FnOnce(&mut Vec<u8>),
    {
        let mut locked_mem = self.memory.lock();
        if let Some(mem) = locked_mem.as_mut() {
            f(mem)
        }
    }

    /// [`get_pointer`] returns the address of the memory
    /// location if it is still valid else throws a panic
    /// as we want this to always be safe.
    pub fn get_pointer(&self) -> MemoryAllocationResult<*const u8> {
        let memory = self.memory.lock();
        match memory.as_ref() {
            Some(mem) => Ok(mem.as_ptr()),
            None => Err(MemoryAllocationError::NoMemoryAllocation),
        }
    }

    pub fn capacity(&self) -> MemoryAllocationResult<u64> {
        let memory = self.memory.lock();
        match memory.as_ref() {
            Some(mem) => Ok(mem.capacity() as u64),
            None => Err(MemoryAllocationError::NoMemoryAllocation),
        }
    }

    pub fn len(&self) -> MemoryAllocationResult<u64> {
        let memory = self.memory.lock();
        match memory.as_ref() {
            Some(mem) => Ok(mem.len() as u64),
            None => Err(MemoryAllocationError::NoMemoryAllocation),
        }
    }

    pub fn reset(&mut self) {
        let mut memory = self.memory.lock();
        if let Some(mem) = memory.as_mut() {
            mem.clear();
            return;
        }
        memory.replace(Vec::new());
    }

    #[allow(clippy::slow_vector_initialization)]
    pub fn reset_to(&mut self, new_capacity: usize) {
        let mut memory = self.memory.lock();
        if let Some(mem) = memory.as_mut() {
            mem.clear();
            if mem.capacity() < new_capacity {
                let reservation = new_capacity - mem.capacity();
                mem.reserve(reservation);
            }
            return;
        }

        let mut new_mem: Vec<u8> = Vec::with_capacity(new_capacity);
        new_mem.resize(new_capacity, 0);
        memory.replace(new_mem);
    }

    pub fn clear(&mut self) -> MemoryAllocationResult<()> {
        let mut memory = self.memory.lock();
        if let Some(mem) = memory.as_mut() {
            mem.clear();
            return Ok(());
        };
        Err(MemoryAllocationError::NoMemoryAllocation)
    }

    pub fn is_valid_memory(&self) -> bool {
        let memory = self.memory.lock();
        memory.as_ref().is_some()
    }

    pub fn clone_memory(&self) -> MemoryAllocationResult<Vec<u8>> {
        let memory = self.memory.lock();
        match memory.as_ref() {
            Some(mem) => Ok(mem.clone()),
            None => Err(MemoryAllocationError::NoMemoryAllocation),
        }
    }

    pub fn vec_from_memory(&self) -> MemoryAllocationResult<Vec<u8>> {
        self.clone_memory()
    }

    pub fn string_from_memory(&self) -> MemoryAllocationResult<String> {
        let mut memory = self.memory.lock();
        if let Some(mem) = memory.as_mut() {
            return match String::from_utf8(mem.clone()) {
                Ok(content) => Ok(content),
                Err(err) => Err(MemoryAllocationError::NotValidUTF8(err)),
            };
        };
        Err(MemoryAllocationError::NoMemoryAllocation)
    }

    /// [`take`] allows you to both de-allocate the
    /// giving memory allocation and own the underlying
    /// memory slice either for dropping or usage.
    pub fn take(&mut self) -> Option<Vec<u8>> {
        let mut memory = self.memory.lock();
        memory.take()
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
#[derive(Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct MemoryId(pub(crate) u32, pub(crate) u32);

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

pub struct MemoryAllocations {
    allocs: Vec<(u32, MemoryAllocation)>,
    free: Vec<usize>,
}

impl MemoryAllocations {
    pub const fn new() -> Self {
        Self {
            allocs: Vec::new(),
            free: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.allocs.is_empty() && self.free.is_empty()
    }

    pub fn is_allocations_empty(&self) -> bool {
        self.allocs.is_empty()
    }

    pub fn total_free(&self) -> u64 {
        self.free.len() as u64
    }

    pub fn total_allocated(&self) -> u64 {
        self.allocs.len() as u64
    }

    /// [`deallocate`] releases the owned memory allocation as
    /// free for re-use by another desiring party. This means
    /// the giving memory location is forever free for usage.
    #[allow(clippy::cast_possible_truncation)]
    pub fn deallocate(&mut self, memory_id: MemoryId) -> MemoryAllocationResult<()> {
        // if its already in there, just ignore
        if self.free.contains(&(memory_id.0 as usize)) {
            return Ok(());
        }

        let total_allocations = self.allocs.len();
        let potential_location = memory_id.0 as usize;
        if potential_location >= total_allocations {
            return Err(MemoryAllocationError::InvalidAllocationId);
        }

        self.free.push(potential_location);
        Ok(())
    }

    /// [`get`] returns the related [`MemoryAllocation`] object that is related to a
    /// giving [`MemoryId`] to be used.
    pub fn get(&mut self, memory_id: MemoryId) -> MemoryAllocationResult<MemoryAllocation> {
        if !self.free.contains(&(memory_id.0 as usize)) {
            if let Some((generation_id, ref allocation)) = self.allocs.get(memory_id.0 as usize) {
                if *generation_id == memory_id.1 {
                    return Ok(allocation.clone());
                }
            }
        }
        Err(MemoryAllocationError::InvalidAllocationId)
    }

    /// [`allocate`] attempts to allocate a memory location with the
    /// desired capacity returning the pointer and ownership via the
    /// returned [`MemoryId`].
    ///
    /// The receiver of the [`MemoryId`] will forever own that allocation
    /// until the [`Self::deallocate`] method is called to free the
    /// allocation.
    #[allow(clippy::cast_possible_truncation)]
    pub fn allocate(&mut self, desired_capacity: u64) -> MemoryAllocationResult<MemoryId> {
        match self.get_ideal_allocation(desired_capacity)? {
            None => {
                let next_index = self.allocs.len();
                if u32::try_from(next_index).is_err() {
                    return Err(MemoryAllocationError::NoMoreAllocationSlots);
                }

                let next_index_u32 = next_index as u32;

                let allocation =
                    MemoryAllocation::new(Vec::with_capacity(desired_capacity as usize));
                self.allocs.push((0, allocation));

                Ok(MemoryId(next_index_u32, 0))
            }
            Some(index) => {
                let (ref mut generation_id, ref mut allocation) =
                    self.allocs.get_mut(index).unwrap();
                allocation.reset_to(desired_capacity as usize);
                *generation_id += 1;

                Ok(MemoryId(index as u32, *generation_id))
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn get_ideal_allocation(
        &mut self,
        desired_capacity_u: u64,
    ) -> MemoryAllocationResult<Option<usize>> {
        let desired_capacity = desired_capacity_u as usize;
        let mut potential_candidate_index: Option<(usize, usize, usize)> = None;
        for (free_index, memory_index) in self.free.iter().enumerate() {
            if let Some((_, ref allocation)) = self.allocs.get(*memory_index) {
                let memory_capacity = allocation.capacity()? as usize;

                match &potential_candidate_index {
                    None => {
                        // if the capacity diff is less than 100 and more than
                        // desired then we can reuse this immediately.
                        if memory_capacity > desired_capacity {
                            let diff = memory_capacity - desired_capacity;
                            if diff < 100 {
                                potential_candidate_index =
                                    Some((*memory_index, memory_capacity, free_index));
                                break;
                            }
                            potential_candidate_index =
                                Some((*memory_index, memory_capacity, free_index));
                            continue;
                        }

                        let diff = desired_capacity - memory_capacity;

                        // if the difference is just between 10 or 100
                        // then this is a potential location we can use with some
                        // expansion, so we will
                        if diff < 100 {
                            potential_candidate_index =
                                Some((*memory_index, memory_capacity, free_index));
                            continue;
                        }

                        potential_candidate_index =
                            Some((*memory_index, memory_capacity, free_index));
                        break;
                    }
                    Some((_, index_size, _)) => {
                        // if index (candidate) is already bigger than desired capacity
                        // but less than current capacity, then this is ideal since we we
                        // do not wish to use larger memory if not required
                        if *index_size > desired_capacity && *index_size < memory_capacity {
                            continue;
                        }

                        // if the capacity is less than current selected and
                        // even less than desired, just skip it.
                        if memory_capacity < *index_size && memory_capacity < desired_capacity {
                            continue;
                        }

                        // if the new candidate is larger than the previous but
                        // still less than recent, then swap potential candidate with
                        // this index
                        if memory_capacity > *index_size && memory_capacity < desired_capacity {
                            potential_candidate_index =
                                Some((*memory_index, memory_capacity, free_index));
                            continue;
                        }

                        // if we found a capacity bigger than desired but less than current
                        // candidate then swap them, we want to use as close a match as possible.
                        if *index_size > desired_capacity && *index_size > memory_capacity {
                            potential_candidate_index =
                                Some((*memory_index, memory_capacity, free_index));
                            continue;
                        }
                    }
                };
            }
        }

        if let Some((index, _, free_index)) = potential_candidate_index {
            // remove the index from the free list.
            _ = self.free.remove(free_index);
            return Ok(Some(index));
        }

        Ok(None)
    }
}

#[cfg(test)]
mod memory_allocation_tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn can_get_allocator_id() {
        let mut allocator = MemoryAllocations::new();

        let mem1 = allocator.allocate(20).expect("should allocate memory");
        assert_eq!(0, mem1.index());
        assert_eq!(0, mem1.generation());

        assert_eq!(0, mem1.as_u64());
    }

    #[test]
    fn can_dispose_of_an_allocation() {
        let mut allocator = MemoryAllocations::new();

        let mem1 = allocator.allocate(20).expect("should allocate memory");
        assert_eq!(0, mem1.index());
        assert_eq!(0, mem1.generation());
        assert_eq!(0, mem1.as_u64());

        allocator
            .deallocate(mem1.clone())
            .expect("should dispose allocation");

        assert!(
            allocator.get(mem1).is_err(),
            "should fail to get allocation"
        );
    }

    #[test]
    fn can_use_allocator() {
        let mut allocator = MemoryAllocations::new();

        let mem1 = allocator.allocate(20).expect("should allocate memory");
        assert_eq!(0, mem1.index());

        let mem2 = allocator.allocate(30).expect("should allocate memory");
        assert_eq!(1, mem2.index());
    }

    #[test]
    fn can_use_allocated_memory() {
        let mut allocator = MemoryAllocations::new();

        let id = allocator.allocate(20).expect("should allocate memory");
        assert_eq!(0, id.index());

        let memory_slot = allocator.get(id).expect("should be able to find memory id");
        memory_slot.apply(|memo| {
            memo.push(10);
            memo.push(20);
            memo.push(30);
        });

        let content = memory_slot.clone_memory().expect("should clone valid data");
        assert_eq!(vec![10, 20, 30], content);
    }
}
