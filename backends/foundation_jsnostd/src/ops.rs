#![allow(dead_code)]
#![allow(clippy::items_after_test_module)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]

use alloc::boxed::Box;
use alloc::string::{FromUtf16Error, FromUtf8Error, String};
use alloc::sync::Arc;
use alloc::vec::Vec;
use foundation_nostd::spin::Mutex;

use super::{ExternalPointer, Operations, StrLocation, ValueTypes};

pub type MemoryWriterResult<T> = core::result::Result<T, MemoryWriterError>;

#[derive(Debug)]
pub enum MemoryWriterError {
    FailedWrite,
    PreviousUnclosedOperation,
    NotValidUTF8(FromUtf8Error),
    NotValidUTF16(FromUtf16Error),
    AllocationError(MemoryAllocationError),
    UnableToWrite,
    UnexpectedFreeState,
}

impl From<MemoryAllocationError> for MemoryWriterError {
    fn from(value: MemoryAllocationError) -> Self {
        Self::AllocationError(value)
    }
}

impl core::error::Error for MemoryWriterError {}

impl core::fmt::Display for MemoryWriterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// [`BatchEncodable`] defines a trait which allows you implement
/// conversion an underlying binary representation of a Batch
/// operation.
pub trait BatchEncodable {
    /// [`string`] encodes the underlying string
    /// returning the string location information which allows
    /// whatever is calling it
    fn string(&self, data: &str) -> MemoryWriterResult<StrLocation>;

    /// [`data`] provides the underlying related data for
    /// the identified operation.
    fn data(&self, data: &[u8]) -> MemoryWriterResult<()>;

    /// [`end`] indicates the batch encoding can be considered finished and
    /// added to the batch list.
    fn end(self);
}

pub struct CompletedInstructions {
    pub ops_id: MemoryId,
    pub text_id: MemoryId,
}

pub struct Instructions {
    ops_id: MemoryId,
    text_id: MemoryId,
    occupied: Option<Operations>,
    mem: Option<(MemoryAllocation, MemoryAllocation)>,
    consumer: Box<dyn FnOnce(CompletedInstructions) + 'static>,
}

// -- Implements BatchEncodable

impl BatchEncodable for Instructions {
    fn string(&self, data: &str) -> MemoryWriterResult<StrLocation> {
        if self.in_occupied_state() {
            if let Some((_, text)) = &self.mem {
                let text_location = text.len()?;
                let text_length = data.len() as u64;

                text.apply(|mem| {
                    mem.extend(data.as_bytes());
                });

                return Ok(StrLocation(text_location, text_length));
            }
        }

        Err(MemoryWriterError::UnableToWrite)
    }

    fn data(&self, data: &[u8]) -> MemoryWriterResult<()> {
        if self.in_occupied_state() {
            if let Some((ops, _)) = &self.mem {
                ops.apply(|mem| {
                    mem.extend(data);
                });

                return Ok(());
            }
        }
        Err(MemoryWriterError::UnableToWrite)
    }

    fn end(mut self) {
        if let Some((ops, _)) = self.mem.take() {
            ops.apply(|mem| {
                mem.push(Operations::Stop as u8);
            });

            (self.consumer)(CompletedInstructions {
                ops_id: self.ops_id,
                text_id: self.text_id,
            });
        }
    }
}

// -- Operations: checker

impl Instructions {
    pub fn in_occupied_state(&self) -> bool {
        self.occupied.is_some()
    }

    pub fn in_free_state(&self) -> bool {
        self.occupied.is_none()
    }

    pub fn should_be_occupied(&self) -> MemoryWriterResult<()> {
        if self.in_free_state() {
            return Err(MemoryWriterError::UnexpectedFreeState);
        }
        Ok(())
    }
}

// -- Operations that can be batch

impl Instructions {
    /// [`register_function`] is immediate and will call end which will flush
    /// the batch into the underlying Operations.
    pub fn register_function(
        self,
        _allocated_handle: u64,
        _body: &str,
    ) -> MemoryWriterResult<Self> {
        self.should_be_occupied()?;

        Ok(self)
    }
}

// -- Constructors

impl Instructions {
    pub fn new(
        ops_id: MemoryId,
        text_id: MemoryId,
        ops: MemoryAllocation,
        texts: MemoryAllocation,
        consumer: Box<dyn FnOnce(CompletedInstructions) + 'static>,
    ) -> Self {
        Self {
            ops_id,
            text_id,
            consumer,
            occupied: None,
            mem: Some((ops, texts)),
        }
    }

    /// [`begin`] starts a new operation to be encoded into the Instructions set
    /// if a operation was not properly closed then an error
    /// [`MemoryWriterError::PreviousUnclosedOperation`] is returned.
    pub fn begin(mut self) -> MemoryWriterResult<Self> {
        if self.occupied.is_some() {
            return Err(MemoryWriterError::PreviousUnclosedOperation);
        }

        self.occupied.replace(Operations::Begin);
        if let Some((op, _)) = &self.mem {
            op.apply(|mem| {
                mem.push(Operations::Begin as u8);
            });
        }
        Ok(self)
    }
}

/// [`Batchable`] defines a infallible type which can be
/// encoded into a [`BatchEncodable`] implementing type
/// usually a [`Batch`].
pub trait Batchable {
    fn encode(&self, encoder: impl BatchEncodable);
}

pub enum Params<'a> {
    Undefined,
    Null,
    Bool(bool),
    Float32(f32),
    Float64(f64),
    Int32(i32),
    Int64(i64),
    Uint32(u32),
    Uint64(u64),
    String(&'a str),
    Uint32Array(&'a [u32]),
    Uint64Array(&'a [u64]),
    Int32Array(&'a [i32]),
    Int64Array(&'a [i64]),
    Float32Array(&'a [f32]),
    Float64Array(&'a [f64]),
    ExternalReference(&'a ExternalPointer),
}

impl Params<'_> {
    fn value_type(&self) -> ValueTypes {
        match self {
            Params::Bool(_) => ValueTypes::Bool,
            Params::Undefined => ValueTypes::Undefined,
            Params::Null => ValueTypes::Null,
            Params::Float32(_) => ValueTypes::Float32,
            Params::Float64(_) => ValueTypes::Float64,
            Params::Int32(_) => ValueTypes::Int32,
            Params::Int64(_) => ValueTypes::Int64,
            Params::Uint64(_) => ValueTypes::Uint64,
            Params::Uint32(_) => ValueTypes::Uint32,
            Params::String(_) => ValueTypes::String,
            Params::Int32Array(_) => ValueTypes::Int32ArrayBuffer,
            Params::Int64Array(_) => ValueTypes::Int64ArrayBuffer,
            Params::Uint32Array(_) => ValueTypes::Uint32ArrayBuffer,
            Params::Uint64Array(_) => ValueTypes::Uint64ArrayBuffer,
            Params::Float32Array(_) => ValueTypes::Float32ArrayBuffer,
            Params::Float64Array(_) => ValueTypes::Float64ArrayBuffer,
            Params::ExternalReference(_) => ValueTypes::ExternalReference,
        }
    }
}

impl From<f64> for Params<'_> {
    fn from(f: f64) -> Self {
        Params::Float64(f)
    }
}

impl From<i32> for Params<'_> {
    fn from(i: i32) -> Self {
        Params::Int32(i)
    }
}

impl From<usize> for Params<'_> {
    fn from(i: usize) -> Self {
        Params::Float64(i as f64)
    }
}

impl From<i64> for Params<'_> {
    fn from(i: i64) -> Self {
        Params::Int64(i)
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

impl Batchable for Params<'_> {
    fn encode(&self, _encoder: impl BatchEncodable) {
        match self {
            Params::Undefined => {
                todo!()
            }
            Params::Null => todo!(),
            Params::Float64(_) => todo!(),
            Params::String(_) => todo!(),
            Params::Float32Array(_) => todo!(),
            Params::Float64Array(_) => todo!(),
            Params::Bool(_) => todo!(),
            Params::Uint32Array(_) => todo!(),
            Params::ExternalReference(_) => todo!(),
            Params::Float32(_) => todo!(),
            Params::Int32(_) => todo!(),
            Params::Int64(_) => todo!(),
            Params::Uint32(_) => todo!(),
            Params::Uint64(_) => todo!(),
            Params::Uint64Array(_) => todo!(),
            Params::Int32Array(_) => todo!(),
            Params::Int64Array(_) => todo!(),
        }
    }
}

/// [`FuncHandle`] defines a type alias providing more
/// context that this is used to represent the location
/// of a runtime function across WASM boundary.
pub type FuncHandle = u64;

pub struct MemoryAllocation {
    memory: Arc<Mutex<Option<Vec<u8>>>>,
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
            memory: Arc::new(Mutex::new(Some(mem))),
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

    pub fn reset(&self) {
        let mut memory = self.memory.lock();
        if let Some(mem) = memory.as_mut() {
            mem.clear();
            return;
        }
        memory.replace(Vec::new());
    }

    #[allow(clippy::slow_vector_initialization)]
    pub fn reset_to(&self, new_capacity: usize) {
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

    pub fn is_empty(&self) -> MemoryAllocationResult<bool> {
        let mut memory = self.memory.lock();
        if let Some(mem) = memory.as_mut() {
            return Ok(mem.is_empty());
        };
        Err(MemoryAllocationError::NoMemoryAllocation)
    }

    pub fn clear(&self) -> MemoryAllocationResult<()> {
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

    pub fn batch_for(
        &mut self,
        text_capacity: u64,
        operations_capacity: u64,
        consumer: impl FnOnce(CompletedInstructions) + 'static,
    ) -> MemoryAllocationResult<Instructions> {
        let operations_id = self.allocate(operations_capacity)?;
        let operations_buffer = self.get(operations_id.clone())?;

        let text_id = self.allocate(text_capacity)?;
        let text_buffer = self.get(text_id.clone())?;

        Ok(Instructions::new(
            operations_id,
            text_id,
            operations_buffer,
            text_buffer,
            Box::new(consumer),
        ))
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
    pub fn get(&self, memory_id: MemoryId) -> MemoryAllocationResult<MemoryAllocation> {
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
    fn can_allocator_memory() {
        let mut allocator = MemoryAllocations::new();

        let mem1 = allocator.allocate(20).expect("should allocate memory");
        assert_eq!(0, mem1.index());
        assert_eq!(0, mem1.generation());
        assert_eq!(0, mem1.as_u64());
    }

    #[test]
    fn can_get_allocator_id() {
        let mut allocator = MemoryAllocations::new();

        let mem1 = allocator.allocate(20).expect("should allocate memory");
        assert_eq!(0, mem1.index());
        assert_eq!(0, mem1.generation());
        assert_eq!(0, mem1.as_u64());

        _ = allocator.get(mem1).expect("should find allocation");
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

    #[test]
    fn can_clear_allocated_memory() {
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

        memory_slot.clear().expect("clear memory");

        assert!(memory_slot
            .is_empty()
            .expect("should return is_empty state"));
    }
}
