#![allow(dead_code)]
#![allow(clippy::wrong_self_convention)]
#![allow(clippy::items_after_test_module)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use foundation_nostd::comp::basic::Mutex;

use crate::{
    BinaryReaderResult, CompletedInstructions, MemoryAllocationError, MemoryAllocationResult,
    MemoryId, MemoryReaderError, MemoryWriterResult, StrLocation,
};

/// [`MemoryAllocation`] is a thread-safe and copy-cheap handle to a
/// underlying memory location held by a [`Vec<u8>`].
///
/// It is cheap to clone a [`MemoryAllocation`] just be aware it all
/// refers to the same memory location Vec<u8>;
pub struct MemoryAllocation {
    memory: Arc<Mutex<Option<Vec<u8>>>>,
}

impl core::fmt::Debug for MemoryAllocation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "MemoryAllocations")
    }
}

impl core::fmt::Display for MemoryAllocation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "MemoryAllocations")
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

    #[inline]
    pub fn into_with<F, T>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&mut Vec<u8>) -> T,
    {
        let mut locked_mem = self.memory.lock().unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        if let Some(mem) = locked_mem.as_mut() {
            return Some(f(mem));
        }
        None
    }

    #[inline]
    pub fn apply<F>(&self, f: F)
    where
        F: FnOnce(&mut Vec<u8>),
    {
        let mut locked_mem = self.memory.lock().unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        if let Some(mem) = locked_mem.as_mut() {
            f(mem);
        }
    }

    #[inline]
    pub fn as_address(&self) -> MemoryAllocationResult<(*const u8, u64)> {
        let memory = self.memory.lock().unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        match memory.as_ref() {
            Some(mem) => Ok((mem.as_ptr(), mem.len() as u64)),
            None => Err(MemoryAllocationError::NoMemoryAllocation),
        }
    }

    /// [`get_pointer`] returns the address of the memory
    /// location if it is still valid else throws a panic
    /// as we want this to always be safe.
    #[inline]
    pub fn get_pointer(&self) -> MemoryAllocationResult<*const u8> {
        let memory = self.memory.lock().unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        match memory.as_ref() {
            Some(mem) => Ok(mem.as_ptr()),
            None => Err(MemoryAllocationError::NoMemoryAllocation),
        }
    }

    #[inline]
    pub fn capacity(&self) -> MemoryAllocationResult<u64> {
        let memory = self.memory.lock().unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        match memory.as_ref() {
            Some(mem) => Ok(mem.capacity() as u64),
            None => Err(MemoryAllocationError::NoMemoryAllocation),
        }
    }

    #[inline]
    pub fn len(&self) -> MemoryAllocationResult<u64> {
        let memory = self.memory.lock().unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        match memory.as_ref() {
            Some(mem) => Ok(mem.len() as u64),
            None => Err(MemoryAllocationError::NoMemoryAllocation),
        }
    }

    #[inline]
    pub fn reset(&self) {
        let mut memory = self.memory.lock().unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        if let Some(mem) = memory.as_mut() {
            mem.clear();
            return;
        }
        memory.replace(Vec::new());
    }

    #[inline]
    #[allow(clippy::slow_vector_initialization)]
    pub fn reset_to(&self, new_capacity: usize) {
        let mut memory = self.memory.lock().unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        if let Some(mem) = memory.as_mut() {
            mem.clear();
            if mem.capacity() < new_capacity {
                let reservation = new_capacity - mem.capacity();
                mem.reserve(reservation);
            }
            mem.resize(new_capacity, 0);
            return;
        }

        let mut new_mem: Vec<u8> = Vec::with_capacity(new_capacity);
        new_mem.resize(new_capacity, 0);
        memory.replace(new_mem);
    }

    #[inline]
    pub fn is_empty(&self) -> MemoryAllocationResult<bool> {
        let mut memory = self.memory.lock().unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        if let Some(mem) = memory.as_mut() {
            return Ok(mem.is_empty());
        }
        Err(MemoryAllocationError::NoMemoryAllocation)
    }

    #[inline]
    pub fn clear(&self) -> MemoryAllocationResult<()> {
        let mut memory = self.memory.lock().unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        if let Some(mem) = memory.as_mut() {
            mem.clear();
            return Ok(());
        }
        Err(MemoryAllocationError::NoMemoryAllocation)
    }

    #[inline]
    pub fn is_valid_memory(&self) -> bool {
        let memory = self.memory.lock().unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        memory.as_ref().is_some()
    }

    #[inline]
    pub fn clone_memory(&self) -> MemoryAllocationResult<Vec<u8>> {
        let memory = self.memory.lock().unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        match memory.as_ref() {
            Some(mem) => Ok(mem.clone()),
            None => Err(MemoryAllocationError::NoMemoryAllocation),
        }
    }

    #[inline]
    pub fn vec_from_memory(&self) -> MemoryAllocationResult<Vec<u8>> {
        self.clone_memory()
    }

    #[inline]
    pub fn string_from_memory(&self) -> MemoryAllocationResult<String> {
        let mut memory = self.memory.lock().unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        if let Some(mem) = memory.as_mut() {
            return match String::from_utf8(mem.clone()) {
                Ok(content) => Ok(content),
                Err(err) => Err(MemoryReaderError::NotValidUTF8(err).into()),
            };
        }
        Err(MemoryAllocationError::NoMemoryAllocation)
    }

    /// [`take`] allows you to both de-allocate the
    /// giving memory allocation and own the underlying
    /// memory slice either for dropping or usage.
    #[inline]
    pub fn take(&mut self) -> Option<Vec<u8>> {
        let mut memory = self.memory.lock().unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        memory.take()
    }
}

/// [`ToBinary`] provides a basic type which we can encode as
/// plain binary bytes usually in `LittleIndian` encoding.
pub trait ToBinary {
    fn to_binary(&self) -> Vec<u8>;
}

/// [`FromBinary`] provides a basic type to convert from binary
/// data to the defined `T`.
///
/// Please take notice that, since we are using [`bincode`],
/// you have to make sure that your types are encoded in a way
/// they can be decoded.
pub trait FromBinary {
    type T;

    fn from_binary(self, bin: &[u8]) -> BinaryReaderResult<Self::T>;
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

    /// [`end`] is used to mark a portion of the batch as completed.
    /// This allows to encode as much individual instructions
    /// into a single batch so that we can take advantage of
    /// treating a series of instructions as atomic operations
    /// that should roughly depending on host handling be
    /// executed together. This allows the host perform a
    /// all or nothing operation if later desired but that is
    /// beyond the scope here. We just want a way to clearly
    /// articulate the end of a sub-instruction and the start
    /// of another.
    fn end(&self) -> MemoryWriterResult<()>;

    /// [`end`] indicates the batch encoding can be considered finished and
    /// added to the batch list.
    fn stop(self) -> MemoryWriterResult<CompletedInstructions>;
}

/// [`Batchable`] defines a infallible type which can be
/// encoded into a [`BatchEncodable`] implementing type
/// usually a [`Batch`].
pub trait Batchable<'a> {
    /// [`encode`] implements the necessary underlying logic to encode the
    /// [`self`] in this instance into bytes that can be correctly communicated
    /// across with an argument [optimized] (boolean) to indicate if any
    /// optimize should be applied during encoding (if at all possible)
    /// which will improve the compactness wherever possible of the encoded
    /// values.
    fn encode<F>(&self, encoder: &'a F, optimized: bool) -> MemoryWriterResult<()>
    where
        F: BatchEncodable;
}

/// [`MemorySlot`] provides a nicer API handle for memory allocation
/// representing one that contains ops code represented as a `Vec<u8>`
/// and text represent by a `Vec<u8>` of a utf-8 encoded text.
pub struct MemorySlot(MemoryAllocation, MemoryAllocation);

impl MemorySlot {
    pub fn new(ops: MemoryAllocation, text: MemoryAllocation) -> Self {
        Self(ops, text)
    }

    pub fn ops(&self) -> MemoryAllocation {
        self.0.clone()
    }
    pub fn text(&self) -> MemoryAllocation {
        self.1.clone()
    }

    pub fn ops_ref(&self) -> &MemoryAllocation {
        &self.0
    }

    pub fn text_ref(&self) -> &MemoryAllocation {
        &self.1
    }
}

pub struct MemoryAllocations {
    allocs: Vec<(u32, MemoryAllocation)>,
    free: Vec<usize>,
}

impl core::fmt::Debug for MemoryAllocations {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "MemoryAllocations(allocs: {:?}, free: {:?})",
            self.allocs, self.free
        )
    }
}

impl core::fmt::Display for MemoryAllocations {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "MemoryAllocations(allocs: {:?}, free: {:?})",
            self.allocs, self.free
        )
    }
}

impl Default for MemoryAllocations {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryAllocations {
    pub const fn create() -> Self {
        Self {
            allocs: Vec::new(),
            free: Vec::new(),
        }
    }

    pub fn new() -> Self {
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
        match self.get_free_slot()? {
            None => {
                let next_index = self.allocs.len();
                if u32::try_from(next_index).is_err() {
                    return Err(MemoryAllocationError::NoMoreAllocationSlots);
                }

                let next_index_u32 = next_index as u32;

                let vec_mem: Vec<u8> = alloc::vec![0; desired_capacity as usize];
                let allocation = MemoryAllocation::new(vec_mem);
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
    fn get_free_slot(&mut self) -> MemoryAllocationResult<Option<usize>> {
        if !self.free.is_empty() {
            if let Some(index) = self.free.pop() {
                return Ok(Some(index));
            }
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
            .deallocate(mem1)
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
        memory_slot.reset_to(0);

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
        memory_slot.reset_to(0);
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
