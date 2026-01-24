//! Lazy one-time initialization container.
//!
//! This provides a `std::sync::OnceLock`-compatible API for `no_std` environments.

use core::cell::UnsafeCell;
use core::fmt;
use core::mem::MaybeUninit;

use crate::primitives::Once;

/// A synchronization primitive for lazy initialization.
///
/// This allows storing a value that is initialized at most once.
pub struct OnceLock<T> {
    once: Once,
    data: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T: Send + Sync> Send for OnceLock<T> {}
unsafe impl<T: Send + Sync> Sync for OnceLock<T> {}

impl<T> OnceLock<T> {
    /// Creates a new empty `OnceLock`.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            once: Once::new(),
            data: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// Gets the value, returning `None` if not yet initialized.
    #[inline]
    pub fn get(&self) -> Option<&T> {
        if self.once.is_completed() {
            Some(unsafe { (*self.data.get()).assume_init_ref() })
        } else {
            None
        }
    }

    /// Gets the value, initializing it if not yet set.
    ///
    /// # Panics
    ///
    /// Panics if the initialization function panics.
    pub fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        if let Some(value) = self.get() {
            return value;
        }

        self.once.call_once(|| {
            let value = f();
            unsafe {
                (*self.data.get()).write(value);
            }
        });

        self.get().unwrap()
    }

    /// Gets the value, initializing it if not yet set.
    ///
    /// # Panics
    ///
    /// Panics if the initialization function panics.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the initialization function returns `Err`.
    /// On error, the lock remains uninitialized.
    pub fn get_or_try_init<F, E>(&self, f: F) -> Result<&T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        if let Some(value) = self.get() {
            return Ok(value);
        }

        let mut init_error: Option<E> = None;
        #[allow(unused_assignments)]
        let init_fn = || match f() {
            Ok(value) => unsafe {
                (*self.data.get()).write(value);
            },
            Err(e) => {
                init_error = Some(e);
                // Force a panic to poison the Once so we can retry later
                panic!("initialization failed");
            }
        };

        // Try initialization
        self.once.call_once_force(|state| {
            if !state.is_poisoned() {
                init_fn();
            }
        });

        // Check if we have an error from this attempt
        if let Some(e) = init_error {
            return Err(e);
        }

        // If we get here, initialization succeeded
        Ok(self.get().unwrap())
    }
    /// Sets the value if not yet initialized.
    ///
    /// # Errors
    ///
    /// Returns `Ok(())` on success, or `Err(value)` if already initialized.
    ///
    /// # Panics
    ///
    /// May panic if initialization was poisoned.
    pub fn set(&self, value: T) -> Result<(), T> {
        let mut captured_value = Some(value);
        let mut set_ok = false;

        self.once.call_once_force(|state| {
            if !state.is_poisoned() {
                let v = captured_value.take().unwrap();
                unsafe {
                    (*self.data.get()).write(v);
                }
                set_ok = true;
            }
        });

        if set_ok {
            Ok(())
        } else if let Some(v) = captured_value {
            Err(v)
        } else {
            // Once was already completed
            Err(unsafe { core::mem::zeroed() }) // This shouldn't happen
        }
    }

    /// Takes the value, leaving the `OnceLock` empty.
    pub fn take(&mut self) -> Option<T> {
        if self.once.is_completed() {
            self.once = Once::new();
            Some(unsafe { self.data.get_mut().assume_init_read() })
        } else {
            None
        }
    }

    /// Consumes the `OnceLock` and returns the value, if set.
    pub fn into_inner(mut self) -> Option<T> {
        self.take()
    }
}

impl<T> Drop for OnceLock<T> {
    fn drop(&mut self) {
        if self.once.is_completed() {
            unsafe {
                self.data.get_mut().assume_init_drop();
            }
        }
    }
}

impl<T> Default for OnceLock<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for OnceLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.get() {
            Some(value) => f.debug_tuple("OnceLock").field(value).finish(),
            None => f.write_str("OnceLock(<uninitialized>)"),
        }
    }
}

impl<T> From<T> for OnceLock<T> {
    fn from(value: T) -> Self {
        let lock = Self::new();
        let _ = lock.set(value);
        lock
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `WHY`: Validates basic `OnceLock` creation and get
    /// `WHAT`: New `OnceLock` should return None
    #[test]
    fn test_new_and_get() {
        let lock = OnceLock::<i32>::new();
        assert!(lock.get().is_none());
    }

    /// `WHY`: Validates set and get work together
    /// `WHAT`: After set, get should return the value
    #[test]
    fn test_set_and_get() {
        let lock = OnceLock::new();
        assert!(lock.set(42).is_ok());
        assert_eq!(*lock.get().unwrap(), 42);
    }

    /// `WHY`: Validates set only succeeds once
    /// `WHAT`: Second set should return error
    #[test]
    fn test_set_twice() {
        let lock = OnceLock::new();
        assert!(lock.set(42).is_ok());
        match lock.set(43) {
            Err(_) => {} // Expected
            Ok(()) => panic!("Second set should fail"),
        }
    }

    /// `WHY`: Validates `get_or_init` lazy initialization
    /// `WHAT`: Should initialize on first call
    #[test]
    fn test_get_or_init() {
        let lock = OnceLock::new();
        let value = lock.get_or_init(|| 42);
        assert_eq!(*value, 42);

        let value2 = lock.get_or_init(|| 43);
        assert_eq!(*value2, 42); // Still the first value
    }

    /// `WHY`: Validates `get_or_try_init` with success
    /// `WHAT`: Should initialize on first call with Ok result
    #[test]
    fn test_get_or_try_init_ok() {
        let lock = OnceLock::new();
        let value = lock.get_or_try_init(|| Ok::<_, ()>(42)).unwrap();
        assert_eq!(*value, 42);
    }

    /// `WHY`: Validates take removes the value
    /// `WHAT`: After take, get should return None
    #[test]
    fn test_take() {
        let mut lock = OnceLock::new();
        lock.set(42).unwrap();
        assert_eq!(lock.take(), Some(42));
        assert!(lock.get().is_none());
    }

    /// `WHY`: Validates `into_inner` consumes and returns value
    /// `WHAT`: Should return Some(value) if set
    #[test]
    fn test_into_inner() {
        let lock = OnceLock::new();
        lock.set(42).unwrap();
        assert_eq!(lock.into_inner(), Some(42));
    }

    /// `WHY`: Validates From implementation
    /// `WHAT`: From should create initialized `OnceLock`
    #[test]
    fn test_from() {
        let lock = OnceLock::from(42);
        assert_eq!(*lock.get().unwrap(), 42);
    }

    /// `WHY`: Validates Default implementation
    /// `WHAT`: Default should create empty `OnceLock`
    #[test]
    fn test_default() {
        let lock = OnceLock::<i32>::default();
        assert!(lock.get().is_none());
    }

    /// `WHY`: Validates Send bound requirement
    /// `WHAT`: `OnceLock` should be Send when T: Send + Sync
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<OnceLock<i32>>();
    }

    /// `WHY`: Validates Sync bound requirement
    /// `WHAT`: `OnceLock` should be Sync when T: Send + Sync
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<OnceLock<i32>>();
    }
}
