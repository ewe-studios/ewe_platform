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
mod tests {
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
        assert_eq!(limiter.current_usage(), 0);
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
