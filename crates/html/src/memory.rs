use std::borrow::Cow;
use std::cell;
use std::fmt::{self, Debug};
use std::ops::Deref;
use std::rc;
use std::result;
use std::str;

use thiserror::Error;

#[derive(Debug)]
pub struct MemoryLimiter {
    current_usage: usize,
    max: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Error)]
pub enum MemoryErrors {
    #[error("The memory limit has been exceeded")]
    MemoryLimitExceededError,
}

type SharedMemoryLimiter = rc::Rc<cell::RefCell<MemoryLimiter>>;

type MemoryResult<T> = result::Result<T, MemoryErrors>;

impl MemoryLimiter {
    pub fn create_shared(max: usize) -> SharedMemoryLimiter {
        rc::Rc::new(cell::RefCell::new(MemoryLimiter {
            current_usage: 0,
            max,
        }))
    }

    #[inline]
    pub(crate) fn current_usage(&self) -> usize {
        self.current_usage
    }

    #[inline]
    pub fn preallocate(&mut self, by_amount: usize) {
        self.increase_usage(by_amount)
            .expect("allocated memory size amount");
    }

    pub fn decrease_usage(&mut self, by_amount: usize) {
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
#[derive(Debug)]
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

        assert_eq!(err, MemoryErrors::MemoryLimitExceededError)
    }
}
