//! No-std implementation using spin-waiting.
//!
//! This module provides a complete implementation of condition variables and mutexes
//! using atomic operations and spin-waiting for `no_std` contexts.

use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU32, AtomicU8, AtomicUsize, Ordering};
use core::time::Duration;

use crate::primitives::{LockResult, PoisonError, SpinWait, TryLockError, TryLockResult};

use super::WaitTimeoutResult;

// ============================================================================
// CondVarMutex - Spin-lock mutex for condition variables
// ============================================================================

// State encoding:
// Bit 0: LOCKED (1 = locked, 0 = unlocked)
// Bit 1: POISONED (1 = poisoned, 0 = clean)
const UNLOCKED: u8 = 0b00;
const LOCKED: u8 = 0b01;
const POISONED: u8 = 0b10;

/// A mutex specifically designed for use with condition variables (spin-lock).
pub struct CondVarMutex<T: ?Sized> {
    state: AtomicU8,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for CondVarMutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for CondVarMutex<T> {}

/// RAII guard for `CondVarMutex`.
pub struct CondVarMutexGuard<'a, T: ?Sized + 'a> {
    pub(crate) mutex: &'a CondVarMutex<T>,
}

unsafe impl<T: ?Sized + Sync> Sync for CondVarMutexGuard<'_, T> {}

impl<T> CondVarMutex<T> {
    /// Creates a new unlocked mutex.
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            state: AtomicU8::new(UNLOCKED),
            data: UnsafeCell::new(data),
        }
    }

    /// Consumes the mutex and returns the inner value.
    #[inline]
    ///
    /// # Errors
    ///
    /// Returns an error if the mutex is poisoned.
    pub fn into_inner(self) -> LockResult<T> {
        let is_poisoned = self.is_poisoned();
        let data = self.data.into_inner();

        if is_poisoned {
            Err(PoisonError::new(data))
        } else {
            Ok(data)
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// # Errors
    ///
    /// Returns an error if the mutex is poisoned.
    #[inline]
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        let is_poisoned = self.is_poisoned();
        let data = self.data.get_mut();

        if is_poisoned {
            Err(PoisonError::new(data))
        } else {
            Ok(data)
        }
    }
}

impl<T: ?Sized> CondVarMutex<T> {
    /// Acquires the mutex, blocking until it becomes available.
    ///
    /// # Errors
    ///
    /// Returns an error if the mutex is poisoned.
    pub fn lock(&self) -> LockResult<CondVarMutexGuard<'_, T>> {
        let mut spin_wait = SpinWait::new();
        loop {
            let state = self.state.load(Ordering::Relaxed);
            if state & LOCKED == 0
                && self
                    .state
                    .compare_exchange_weak(
                        state,
                        state | LOCKED,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    )
                    .is_ok()
            {
                let guard = CondVarMutexGuard { mutex: self };
                return if state & POISONED != 0 {
                    Err(PoisonError::new(guard))
                } else {
                    Ok(guard)
                };
            }
            spin_wait.spin();
        }
    }

    /// Attempts to acquire the mutex without blocking.
    ///
    /// # Errors
    ///
    /// Returns `TryLockError::WouldBlock` if the mutex is already locked,
    /// or `TryLockError::Poisoned` if the mutex is poisoned.
    pub fn try_lock(&self) -> TryLockResult<CondVarMutexGuard<'_, T>> {
        let state = self.state.load(Ordering::Relaxed);
        if state & LOCKED != 0 {
            return Err(TryLockError::WouldBlock);
        }

        if self
            .state
            .compare_exchange(state, state | LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            let guard = CondVarMutexGuard { mutex: self };
            if state & POISONED != 0 {
                Err(TryLockError::Poisoned(PoisonError::new(guard)))
            } else {
                Ok(guard)
            }
        } else {
            Err(TryLockError::WouldBlock)
        }
    }

    /// Returns whether this mutex is poisoned.
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        self.state.load(Ordering::Relaxed) & POISONED != 0
    }

    /// Unlocks the mutex.
    ///
    /// # Safety
    ///
    /// This must only be called by the thread that currently holds the lock.
    #[inline]
    pub(crate) unsafe fn unlock(&self) {
        let state = self.state.load(Ordering::Relaxed);
        self.state.store(state & !LOCKED, Ordering::Release);
    }

    /// Marks the mutex as poisoned.
    #[inline]
    #[allow(dead_code)]
    fn poison(&self) {
        self.state.fetch_or(POISONED, Ordering::Relaxed);
    }
}

impl<'a, T: ?Sized> CondVarMutexGuard<'a, T> {
    /// Returns a reference to the parent mutex.
    #[inline]
    #[must_use]
    pub fn mutex(&self) -> &'a CondVarMutex<T> {
        self.mutex
    }
}

impl<T: ?Sized> Deref for CondVarMutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T: ?Sized> DerefMut for CondVarMutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T: ?Sized> Drop for CondVarMutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.mutex.unlock();
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for CondVarMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for CondVarMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for CondVarMutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("CondVarMutex");
        match self.try_lock() {
            Ok(guard) => d.field("data", &&*guard),
            Err(TryLockError::Poisoned(ref e)) => d.field("data", &&**e.get_ref()),
            Err(TryLockError::WouldBlock) => d.field("data", &format_args!("<locked>")),
        };
        d.field("poisoned", &self.is_poisoned());
        d.finish_non_exhaustive()
    }
}

impl<T: Default> Default for CondVarMutex<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

// ============================================================================
// RawCondVarMutex - Simple non-poisoning mutex
// ============================================================================

use core::sync::atomic::AtomicBool;

/// A simple non-poisoning mutex for condition variables (spin-lock).
pub struct RawCondVarMutex<T: ?Sized> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for RawCondVarMutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for RawCondVarMutex<T> {}

/// RAII guard for `RawCondVarMutex`.
pub struct RawCondVarMutexGuard<'a, T: ?Sized + 'a> {
    pub(crate) mutex: &'a RawCondVarMutex<T>,
}

unsafe impl<T: ?Sized + Sync> Sync for RawCondVarMutexGuard<'_, T> {}

impl<T> RawCondVarMutex<T> {
    /// Creates a new unlocked mutex.
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Consumes the mutex and returns the inner value.
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }

    /// Returns a mutable reference to the underlying data.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }
}

impl<T: ?Sized> RawCondVarMutex<T> {
    /// Acquires the mutex, blocking until it becomes available.
    pub fn lock(&self) -> RawCondVarMutexGuard<'_, T> {
        let mut spin_wait = SpinWait::new();
        loop {
            if self
                .locked
                .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return RawCondVarMutexGuard { mutex: self };
            }
            spin_wait.spin();
        }
    }

    /// Attempts to acquire the lock without blocking.
    #[inline]
    pub fn try_lock(&self) -> Option<RawCondVarMutexGuard<'_, T>> {
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(RawCondVarMutexGuard { mutex: self })
        } else {
            None
        }
    }

    /// Unlocks the mutex.
    ///
    /// # Safety
    ///
    /// This must only be called by the thread that currently holds the lock.
    #[inline]
    pub(crate) unsafe fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

impl<'a, T: ?Sized> RawCondVarMutexGuard<'a, T> {
    /// Returns a reference to the parent mutex.
    #[inline]
    #[must_use]
    pub fn mutex(&self) -> &'a RawCondVarMutex<T> {
        self.mutex
    }
}

impl<T: ?Sized> Deref for RawCondVarMutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T: ?Sized> DerefMut for RawCondVarMutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T: ?Sized> Drop for RawCondVarMutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.mutex.unlock();
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RawCondVarMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for RawCondVarMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RawCondVarMutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("RawCondVarMutex");
        match self.try_lock() {
            Some(guard) => d.field("data", &&*guard),
            None => d.field("data", &format_args!("<locked>")),
        };
        d.finish_non_exhaustive()
    }
}

impl<T: Default> Default for RawCondVarMutex<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

// ============================================================================
// CondVar - Spin-wait condition variables
// ============================================================================

// State encoding (32-bit):
// Bits 0-29: Waiting thread count
// Bit 30: Notification pending flag
const WAITER_MASK: u32 = (1 << 30) - 1;
const NOTIFY_FLAG: u32 = 1 << 30;

/// A condition variable with poisoning support (spin-wait implementation).
pub struct CondVar {
    state: AtomicU32,
    generation: AtomicUsize,
}

impl CondVar {
    /// Creates a new condition variable.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(0),
            generation: AtomicUsize::new(0),
        }
    }

    /// Blocks the current thread until notified.
    ///
    /// # Errors
    ///
    /// Returns an error if the mutex was poisoned before or after waiting.
    pub fn wait<'a, T>(
        &self,
        guard: CondVarMutexGuard<'a, T>,
    ) -> LockResult<CondVarMutexGuard<'a, T>> {
        let mutex = guard.mutex();
        let was_poisoned = mutex.is_poisoned();

        self.state.fetch_add(1, Ordering::Relaxed);
        let gen = self.generation.load(Ordering::Acquire);
        drop(guard);

        // Spin-wait for generation change
        let mut spin_wait = SpinWait::new();
        loop {
            let new_gen = self.generation.load(Ordering::Acquire);
            let state = self.state.load(Ordering::Acquire);

            if new_gen != gen || state & NOTIFY_FLAG != 0 {
                break;
            }

            spin_wait.spin();
        }

        self.state.fetch_sub(1, Ordering::Relaxed);
        let guard = mutex.lock()?;

        if was_poisoned || mutex.is_poisoned() {
            Err(PoisonError::new(guard))
        } else {
            Ok(guard)
        }
    }

    /// Waits on this condition variable with a predicate.
    ///
    /// # Errors
    ///
    /// Returns an error if the mutex is poisoned.
    pub fn wait_while<'a, T, F>(
        &self,
        mut guard: CondVarMutexGuard<'a, T>,
        mut condition: F,
    ) -> LockResult<CondVarMutexGuard<'a, T>>
    where
        F: FnMut(&mut T) -> bool,
    {
        while condition(&mut *guard) {
            guard = self.wait(guard)?;
        }
        Ok(guard)
    }

    /// Waits on this condition variable with a timeout.
    ///
    /// # Errors
    ///
    /// Returns an error if the mutex was poisoned before or after waiting.
    pub fn wait_timeout<'a, T>(
        &self,
        guard: CondVarMutexGuard<'a, T>,
        dur: Duration,
    ) -> LockResult<(CondVarMutexGuard<'a, T>, WaitTimeoutResult)> {
        let mutex = guard.mutex();
        let was_poisoned = mutex.is_poisoned();

        self.state.fetch_add(1, Ordering::Relaxed);
        let gen = self.generation.load(Ordering::Acquire);
        drop(guard);

        let timed_out = self.wait_timeout_impl(gen, dur);

        self.state.fetch_sub(1, Ordering::Relaxed);
        let guard = mutex.lock();
        let result = WaitTimeoutResult::new(timed_out);

        match guard {
            Ok(g) => {
                if was_poisoned || mutex.is_poisoned() {
                    Err(PoisonError::new((g, result)))
                } else {
                    Ok((g, result))
                }
            }
            Err(e) => Err(PoisonError::new((e.into_inner(), result))),
        }
    }

    fn wait_timeout_impl(&self, gen: usize, dur: Duration) -> bool {
        let max_spins = (dur.as_micros() / 10).max(1) as usize;
        let mut spin_wait = SpinWait::new();

        for _ in 0..max_spins {
            let new_gen = self.generation.load(Ordering::Acquire);
            let state = self.state.load(Ordering::Acquire);

            if new_gen != gen || state & NOTIFY_FLAG != 0 {
                return false;
            }

            spin_wait.spin();
        }

        let new_gen = self.generation.load(Ordering::Acquire);
        let state = self.state.load(Ordering::Acquire);
        new_gen == gen && state & NOTIFY_FLAG == 0
    }

    /// Waits on this condition variable with a timeout and a predicate.
    ///
    /// # Errors
    ///
    /// Returns an error if the mutex is poisoned.
    pub fn wait_timeout_while<'a, T, F>(
        &self,
        mut guard: CondVarMutexGuard<'a, T>,
        dur: Duration,
        mut condition: F,
    ) -> LockResult<(CondVarMutexGuard<'a, T>, WaitTimeoutResult)>
    where
        F: FnMut(&mut T) -> bool,
    {
        loop {
            if !condition(&mut *guard) {
                return Ok((guard, WaitTimeoutResult::new(false)));
            }

            let remaining = dur;
            if remaining.as_nanos() == 0 {
                return Ok((guard, WaitTimeoutResult::new(true)));
            }

            let (g, timeout_result) = self.wait_timeout(guard, remaining)?;
            guard = g;

            if timeout_result.timed_out() {
                return Ok((guard, timeout_result));
            }
        }
    }

    /// Wakes up one blocked thread.
    #[inline]
    pub fn notify_one(&self) {
        let state = self.state.load(Ordering::Acquire);
        if state & WAITER_MASK > 0 {
            self.generation.fetch_add(1, Ordering::Release);
        }
    }

    /// Wakes up all blocked threads.
    #[inline]
    pub fn notify_all(&self) {
        let state = self.state.load(Ordering::Acquire);
        if state & WAITER_MASK > 0 {
            self.generation.fetch_add(1, Ordering::Release);
        }
    }
}

impl Default for CondVar {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CondVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CondVar").finish_non_exhaustive()
    }
}

/// A condition variable without poisoning support (spin-wait implementation).
pub struct CondVarNonPoisoning {
    state: AtomicU32,
    generation: AtomicUsize,
}

impl CondVarNonPoisoning {
    /// Creates a new non-poisoning condition variable.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(0),
            generation: AtomicUsize::new(0),
        }
    }

    /// Blocks the current thread until notified.
    pub fn wait<'a, T>(&self, guard: RawCondVarMutexGuard<'a, T>) -> RawCondVarMutexGuard<'a, T> {
        let mutex = guard.mutex();
        self.state.fetch_add(1, Ordering::Relaxed);
        let gen = self.generation.load(Ordering::Acquire);
        drop(guard);

        let mut spin_wait = SpinWait::new();
        loop {
            let new_gen = self.generation.load(Ordering::Acquire);
            if new_gen != gen {
                break;
            }
            spin_wait.spin();
        }

        self.state.fetch_sub(1, Ordering::Relaxed);
        mutex.lock()
    }

    /// Waits with a predicate.
    pub fn wait_while<'a, T, F>(
        &self,
        mut guard: RawCondVarMutexGuard<'a, T>,
        mut condition: F,
    ) -> RawCondVarMutexGuard<'a, T>
    where
        F: FnMut(&mut T) -> bool,
    {
        while condition(&mut *guard) {
            guard = self.wait(guard);
        }
        guard
    }

    /// Waits with a timeout.
    pub fn wait_timeout<'a, T>(
        &self,
        guard: RawCondVarMutexGuard<'a, T>,
        dur: Duration,
    ) -> (RawCondVarMutexGuard<'a, T>, WaitTimeoutResult) {
        let mutex = guard.mutex();
        self.state.fetch_add(1, Ordering::Relaxed);
        let gen = self.generation.load(Ordering::Acquire);
        drop(guard);

        let timed_out = self.wait_timeout_impl(gen, dur);

        self.state.fetch_sub(1, Ordering::Relaxed);
        let guard = mutex.lock();
        (guard, WaitTimeoutResult::new(timed_out))
    }

    fn wait_timeout_impl(&self, gen: usize, dur: Duration) -> bool {
        let max_spins = (dur.as_micros() / 10).max(1) as usize;
        let mut spin_wait = SpinWait::new();

        for _ in 0..max_spins {
            let new_gen = self.generation.load(Ordering::Acquire);
            if new_gen != gen {
                return false;
            }
            spin_wait.spin();
        }

        let new_gen = self.generation.load(Ordering::Acquire);
        new_gen == gen
    }

    /// Waits with a timeout and predicate.
    pub fn wait_timeout_while<'a, T, F>(
        &self,
        mut guard: RawCondVarMutexGuard<'a, T>,
        dur: Duration,
        mut condition: F,
    ) -> (RawCondVarMutexGuard<'a, T>, WaitTimeoutResult)
    where
        F: FnMut(&mut T) -> bool,
    {
        loop {
            if !condition(&mut *guard) {
                return (guard, WaitTimeoutResult::new(false));
            }

            let remaining = dur;
            if remaining.as_nanos() == 0 {
                return (guard, WaitTimeoutResult::new(true));
            }

            let (g, timeout_result) = self.wait_timeout(guard, remaining);
            guard = g;

            if timeout_result.timed_out() {
                return (guard, timeout_result);
            }
        }
    }

    /// Wakes up one blocked thread.
    #[inline]
    pub fn notify_one(&self) {
        let state = self.state.load(Ordering::Acquire);
        if state & WAITER_MASK > 0 {
            self.generation.fetch_add(1, Ordering::Release);
        }
    }

    /// Wakes up all blocked threads.
    #[inline]
    pub fn notify_all(&self) {
        let state = self.state.load(Ordering::Acquire);
        if state & WAITER_MASK > 0 {
            self.generation.fetch_add(1, Ordering::Release);
        }
    }
}

impl Default for CondVarNonPoisoning {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CondVarNonPoisoning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CondVarNonPoisoning")
            .finish_non_exhaustive()
    }
}

/// A condition variable for use with `RwLocks` (spin-wait implementation).
pub struct RwLockCondVar {
    state: AtomicU32,
    generation: AtomicUsize,
}

impl RwLockCondVar {
    /// Creates a new `RwLock` condition variable.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(0),
            generation: AtomicUsize::new(0),
        }
    }

    /// Wakes up one blocked thread.
    #[inline]
    pub fn notify_one(&self) {
        let state = self.state.load(Ordering::Acquire);
        if state & WAITER_MASK > 0 {
            self.generation.fetch_add(1, Ordering::Release);
        }
    }

    /// Wakes up all blocked threads.
    #[inline]
    pub fn notify_all(&self) {
        let state = self.state.load(Ordering::Acquire);
        if state & WAITER_MASK > 0 {
            self.generation.fetch_add(1, Ordering::Release);
        }
    }
}

impl Default for RwLockCondVar {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for RwLockCondVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RwLockCondVar").finish_non_exhaustive()
    }
}
