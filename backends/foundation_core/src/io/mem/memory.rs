use std::cell;
use std::fmt::Debug;
use std::mem::size_of;
use std::ops::{Deref, Index, RangeBounds};
use std::rc;
use std::result;
use std::vec::Drain;

use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Copy, Error)]
pub enum MemoryErrors {
    #[error("The memory limit has been exceeded")]
    MemoryLimitExceededError,
}

type SharedMemoryLimiter = rc::Rc<cell::RefCell<MemoryLimiter>>;

type MemoryResult<T> = result::Result<T, MemoryErrors>;

#[derive(Debug, Clone)]
pub struct MemoryLimiter {
    current_usage: usize,
    max: usize,
}

impl MemoryLimiter {
    #[must_use] 
    pub fn create_shared(max: usize) -> SharedMemoryLimiter {
        rc::Rc::new(cell::RefCell::new(MemoryLimiter {
            current_usage: 0,
            max,
        }))
    }

    #[must_use] 
    pub fn non_shared(max: usize) -> MemoryLimiter {
        Self {
            current_usage: 0,
            max,
        }
    }

    #[inline]
    pub fn set_capacity(&mut self, new_max: usize) {
        self.max = new_max;
    }

    #[inline]
    #[must_use] 
    pub fn capacity(&self) -> usize {
        self.max
    }

    #[inline]
    #[must_use] 
    pub fn current_usage(&self) -> usize {
        self.current_usage
    }

    #[inline]
    pub fn preallocate(&mut self, by_amount: usize) {
        self.increase_usage(by_amount)
            .expect("allocated memory size amount");
    }

    #[inline]
    pub fn decrease_usage(&mut self, by_amount: usize) {
        if self.current_usage == 0 {
            return;
        }
        self.current_usage -= by_amount;
    }

    #[inline]
    pub fn increase_usage(&mut self, by_amount: usize) -> MemoryResult<()> {
        self.current_usage += by_amount;

        if self.current_usage > self.max {
            return Err(MemoryErrors::MemoryLimitExceededError);
        }

        Ok(())
    }
}

#[cfg(test)]
mod memory_limiter_tests {
    use super::*;

    #[test]
    fn cant_expand_usage_pass_limits() {
        let limiter_rc = MemoryLimiter::create_shared(10);
        let mut limiter = limiter_rc.borrow_mut();

        assert_eq!(limiter.current_usage(), 0);

        assert!(matches!(
            limiter.increase_usage(15),
            MemoryResult::Err(MemoryErrors::MemoryLimitExceededError)
        ));
        assert_eq!(limiter.current_usage(), 15);
    }

    #[test]
    fn can_get_current_usage() {
        let limiter_rc = MemoryLimiter::create_shared(10);
        let mut limiter = limiter_rc.borrow_mut();

        assert_eq!(limiter.current_usage(), 0);

        assert!(matches!(limiter.increase_usage(5), MemoryResult::Ok(_)));
        assert_eq!(limiter.current_usage(), 5);
    }
}

/// Arena provides a pre-allocated memory that can grow and wont de-allocates
/// will still being used and in the liftetime of
#[derive(Debug, Clone)]
pub struct Arena {
    limiter: SharedMemoryLimiter,
    data: Vec<u8>,
}

impl Arena {
    pub fn new(limiter: SharedMemoryLimiter, preallocate: usize) -> Self {
        limiter.borrow_mut().preallocate(preallocate);

        Self {
            limiter,
            data: Vec::with_capacity(preallocate),
        }
    }

    #[inline]
    #[must_use] 
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    #[must_use] 
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    // shift moves the data from the starting point of the amount basically using
    // it has the starting index to copy the data to teh start of the internal memory location
    // truncating any data after copying.
    pub fn shift(&mut self, by_amount: usize) {
        self.data.copy_within(by_amount.., 0);
        self.data.truncate(self.data.len() - by_amount);
    }

    pub fn init_with(&mut self, slice: &[u8]) -> MemoryResult<()> {
        self.data.clear();
        self.append(slice)
    }

    pub fn append(&mut self, slice: &[u8]) -> MemoryResult<()> {
        let new_len = self.data.len() + slice.len();
        let capacity = self.data.capacity();

        // if the new_length is higher than current capacity then
        // we need to grow with the difference.
        if new_len > capacity {
            let diff_size = new_len - capacity;

            // NOTE: Vec::reserve_exact is approximate and does not guarantee
            // exact capacity
            self.limiter.borrow_mut().increase_usage(diff_size)?;

            // If we pre-allocate well, this will rarely run and its better than double
            // capacity strategy to better manage memory as that could cause
            // us to exhaust memory faster
            self.data.reserve_exact(slice.len());
        }

        self.data.extend_from_slice(slice);

        Ok(())
    }
}

impl Deref for Arena {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.data.as_slice()
    }
}

impl Index<usize> for Arena {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        self.limiter.borrow_mut().decrease_usage(self.data.len());
    }
}

#[cfg(test)]
mod arena_tests {

    use super::*;

    #[test]
    fn can_create_arena() {
        let limiter = MemoryLimiter::create_shared(10);
        let mut arena = Arena::new(rc::Rc::clone(&limiter), 2);

        arena.append(&[1, 2]).unwrap();
        assert_eq!(arena.bytes(), &[1, 2]);
        assert_eq!(limiter.borrow().current_usage(), 2);

        arena.append(&[3, 4]).unwrap();
        assert_eq!(arena.bytes(), &[1, 2, 3, 4]);
        assert_eq!(limiter.borrow().current_usage(), 4);

        arena.append(&[5, 6, 7, 8, 9, 10]).unwrap();
        assert_eq!(arena.bytes(), &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(limiter.borrow().current_usage(), 10);

        let err = arena.append(&[11]).unwrap_err();

        assert_eq!(err, MemoryErrors::MemoryLimitExceededError);
    }
}

#[must_use] 
pub fn calculate_size_for<T>(by_multiple: Option<usize>) -> usize {
    if let Some(by_value) = by_multiple {
        return size_of::<T>() * by_value;
    }
    size_of::<T>()
}

/// `TypeArena<T>` provides a pre-allocated memory that can grow
/// and store a specific element of a given type.
/// It grows with usage and continuously keeps the memory
/// available for the lifetime of the [`T`].
#[derive(Debug, Clone)]
pub struct TypeArena<T> {
    limiter: SharedMemoryLimiter,
    data: Vec<T>,
}

impl<T> TypeArena<T> {
    pub fn new(limiter: SharedMemoryLimiter) -> Self {
        Self {
            limiter,
            data: vec![],
        }
    }

    pub fn preallocate(limiter: SharedMemoryLimiter, multiple_of: usize) -> Self {
        let preallocate = calculate_size_for::<T>(Some(multiple_of));
        limiter.borrow_mut().preallocate(preallocate);

        Self {
            limiter,
            data: Vec::with_capacity(preallocate),
        }
    }

    pub fn push(&mut self, element: T) -> MemoryResult<()> {
        let by_amount = calculate_size_for::<T>(None);
        self.limiter.borrow_mut().increase_usage(by_amount)?;
        self.data.push(element);
        Ok(())
    }

    #[inline]
    pub fn first_mut(&mut self) -> Option<&mut T> {
        self.data.first_mut()
    }

    #[inline]
    #[must_use] 
    pub fn first(&self) -> Option<&T> {
        self.data.first()
    }

    #[inline]
    pub fn last_mut(&mut self) -> Option<&mut T> {
        self.data.last_mut()
    }

    #[inline]
    #[must_use] 
    pub fn last(&self) -> Option<&T> {
        self.data.last()
    }

    #[inline]
    #[must_use] 
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    #[inline]
    #[must_use] 
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[inline]
    #[must_use] 
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Removes and returns the last item from the `Vector`
    /// decreasing the memory usage of the `TArea<T>`,
    pub fn drain_last(&mut self) -> Option<T> {
        let by_amount = calculate_size_for::<T>(None);
        self.limiter.borrow_mut().decrease_usage(by_amount);
        self.data.pop()
    }

    /// Returns a draining iterator that removes the
    /// specified range in the underlying vector
    /// yields the removed items.
    pub fn drain<R>(&mut self, range: R) -> Drain<'_, T>
    where
        R: RangeBounds<usize>,
    {
        use std::ops::Bound::{Included, Excluded, Unbounded};

        let start = match range.start_bound() {
            Included(&n) => n,
            Excluded(&n) => n + 1,
            Unbounded => 0,
        };

        let end = match range.end_bound() {
            Included(&n) => n + 1,
            Excluded(&n) => n,
            Unbounded => self.len(),
        };

        self.limiter
            .borrow_mut()
            .decrease_usage(calculate_size_for::<T>(Some(end - start)));

        self.data.drain(range)
    }
}

impl<T> Deref for TypeArena<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.data.as_slice()
    }
}

impl<T> Index<usize> for TypeArena<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl<T> Drop for TypeArena<T> {
    fn drop(&mut self) {
        self.limiter
            .borrow_mut()
            .decrease_usage(calculate_size_for::<T>(Some(self.data.len())));
    }
}

#[cfg(test)]
mod type_area_tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn current_usage() {
        {
            let limiter = MemoryLimiter::create_shared(10);
            let mut vec_u8: TypeArena<u8> = TypeArena::new(Rc::clone(&limiter));

            vec_u8.push(1).unwrap();
            vec_u8.push(2).unwrap();
            assert_eq!(limiter.borrow().current_usage(), 2);
        }

        {
            let limiter = MemoryLimiter::create_shared(10);
            let mut vec_u32: TypeArena<u32> = TypeArena::new(Rc::clone(&limiter));

            vec_u32.push(1).unwrap();
            vec_u32.push(2).unwrap();
            assert_eq!(limiter.borrow().current_usage(), 8);
        }
    }

    #[test]
    fn max_limit() {
        let limiter = MemoryLimiter::create_shared(2);
        let mut vector: TypeArena<u8> = TypeArena::new(Rc::clone(&limiter));

        vector.push(1).unwrap();
        vector.push(2).unwrap();

        let err = vector.push(3).unwrap_err();

        assert_eq!(err, MemoryErrors::MemoryLimitExceededError);
    }

    #[test]
    fn drop() {
        let limiter = MemoryLimiter::create_shared(1);

        {
            let mut vector: TypeArena<u8> = TypeArena::new(Rc::clone(&limiter));

            vector.push(1).unwrap();
            assert_eq!(limiter.borrow().current_usage(), 1);
        }

        assert_eq!(limiter.borrow().current_usage(), 0);
    }

    #[test]
    fn drain() {
        let limiter = MemoryLimiter::create_shared(10);
        let mut vector: TypeArena<u8> = TypeArena::new(Rc::clone(&limiter));

        vector.push(1).unwrap();
        vector.push(2).unwrap();
        vector.push(3).unwrap();
        assert_eq!(limiter.borrow().current_usage(), 3);

        vector.drain(0..3);
        assert_eq!(limiter.borrow().current_usage(), 0);

        vector.push(1).unwrap();
        vector.push(2).unwrap();
        vector.push(3).unwrap();
        vector.push(4).unwrap();
        assert_eq!(limiter.borrow().current_usage(), 4);

        vector.drain(1..=2);
        assert_eq!(limiter.borrow().current_usage(), 2);
    }
}

pub type ReferencedType<T> = rc::Rc<cell::RefCell<T>>;

pub type GeneratorFunc<T> = dyn Fn(&mut ArenaPool<T>) -> T;

pub trait Resettable {
    fn reset(&mut self);
}

pub trait PoolGenerator<T: Resettable> {
    fn generate(&self, pool: &mut ArenaPool<T>) -> T;
}

pub struct FnGenerator<T: Resettable> {
    handler: Box<GeneratorFunc<T>>,
}

impl<T: Resettable> FnGenerator<T> {
    pub fn new(handler: impl Fn(&mut ArenaPool<T>) -> T + 'static) -> Self {
        Self {
            handler: Box::new(handler),
        }
    }
}

impl<T: Resettable> PoolGenerator<T> for FnGenerator<T> {
    fn generate(&self, pool: &mut ArenaPool<T>) -> T {
        (self.handler)(pool)
    }
}

/// `ArenaPool` provides a single-threaded object pool which allows us to easily
/// generate a trackable and reusable set of objects that can be freely
/// allocated based on the underlying memory limits as dictated by the
/// `SharedMemoryLimiter`.
///
/// This is not thread-safe.
#[derive(Clone)]
pub struct ArenaPool<T: Resettable> {
    arena: TypeArena<T>,
    tracker: MemoryLimiter,
    limiter: SharedMemoryLimiter,
    generator: rc::Rc<dyn PoolGenerator<T>>,
}

pub type SharedArenaPool<T> = rc::Rc<cell::RefCell<ArenaPool<T>>>;

impl<T: Resettable> ArenaPool<T> {
    pub fn create_shared(
        limiter: SharedMemoryLimiter,
        gen: impl PoolGenerator<T> + 'static,
    ) -> SharedArenaPool<T> {
        let tracker = MemoryLimiter::non_shared(limiter.borrow().capacity());
        rc::Rc::new(cell::RefCell::new(Self {
            tracker,
            generator: rc::Rc::new(gen),
            limiter: rc::Rc::clone(&limiter),
            arena: TypeArena::new(rc::Rc::clone(&limiter)),
        }))
    }

    pub fn new(limiter: SharedMemoryLimiter, gen: impl PoolGenerator<T> + 'static) -> Self {
        let tracker = MemoryLimiter::non_shared(limiter.borrow().capacity());
        Self {
            tracker,
            generator: rc::Rc::new(gen),
            limiter: rc::Rc::clone(&limiter),
            arena: TypeArena::new(rc::Rc::clone(&limiter)),
        }
    }

    #[inline]
    #[must_use] 
    pub fn remaining_allocation(&self) -> usize {
        self.limiter.borrow().capacity() - self.tracker.current_usage()
    }

    #[inline]
    #[must_use] 
    pub fn allocated(&self) -> usize {
        self.tracker.current_usage()
    }

    #[inline]
    #[must_use] 
    pub fn capacity(&self) -> usize {
        self.limiter.borrow().capacity()
    }

    #[inline]
    pub fn deallocate(&mut self, mut elem: T) {
        self.tracker.set_capacity(self.limiter.borrow().capacity());
        elem.reset();
        self.arena.push(elem).expect("should push in element");
        self.tracker.decrease_usage(calculate_size_for::<T>(None));
    }

    #[inline]
    pub fn allocate(&mut self) -> MemoryResult<T> {
        self.tracker.set_capacity(self.limiter.borrow().capacity());

        // if we have items in the arena, meaning
        // we can reuse a previous structure then pull that up and send it
        // out as return value to the requester.
        if !self.arena.is_empty() {
            self.tracker.increase_usage(calculate_size_for::<T>(None))?;

            let elem = self.arena.drain_last().expect("should have element");
            return Ok(elem);
        }

        self.tracker.increase_usage(calculate_size_for::<T>(None))?;

        let gen = self.generator.clone();
        let elem = gen.generate(self);
        Ok(elem)
    }
}

#[cfg(test)]
mod arena_pool_tests {
    use super::*;

    type ResettableU8 = &'static u8;

    impl Resettable for ResettableU8 {
        fn reset(&mut self) {
            *self = &0;
        }
    }

    #[test]
    fn test_arena_pool() {
        let limiter = MemoryLimiter::create_shared(800);
        let mut pool: ArenaPool<ResettableU8> = ArenaPool::new(
            limiter,
            FnGenerator::new(|_| {
                let new_value: ResettableU8 = &0;
                new_value
            }),
        );

        assert_eq!(pool.allocated(), 0);

        let mut _my_number = pool.allocate().unwrap();

        assert_eq!(pool.allocated(), 8);

        _my_number = &1;

        assert_eq!(*_my_number, 1);

        pool.deallocate(_my_number);

        _ = _my_number;
    }

    #[test]
    fn test_arena_pool_limits() {
        let limiter = MemoryLimiter::create_shared(8);
        let mut pool: ArenaPool<ResettableU8> = ArenaPool::new(
            limiter,
            FnGenerator::new(|_| {
                let new_value: ResettableU8 = &0;
                new_value
            }),
        );

        assert_eq!(pool.allocated(), 0);

        _ = pool.allocate().unwrap();

        assert_eq!(pool.allocated(), 8);

        assert!(matches!(pool.allocate(), MemoryResult::Err(_)));
    }
}
