// Implements an Lock notification primitive usable in threads.

use foundation_nostd::comp::condvar_comp::{CondVar, Mutex};
use std::panic::{RefUnwindSafe, UnwindSafe};

use super::Waker;

/// `LockState` defines the underlying state of a CondVar based
/// locks which will allow us sleep a thread silently without eating up
/// CPU cycles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LockState {
    Free,
    Locked,
    Released,
}

/// `LockSignal` allows us sleep a thread or process until a signal
/// gets delivered via it's underlying `CondVar`.
pub struct LockSignal {
    /// The condition variable used to wait on an event,
    /// also provides a way to awake a sleeping thread.
    event: CondVar,

    /// The mutex used to protect the event.
    ///
    /// Inside is a state indicative of the current state of the
    /// the lock signal.
    lock: Mutex<LockState>,
}

// SAFETY: LockSignal is safe to use across unwind boundaries.
// The internal Mutex and CondVar are designed to maintain consistency
// even in the presence of panics, and LockState is a simple enum.
impl UnwindSafe for LockSignal {}
impl RefUnwindSafe for LockSignal {}

pub(crate) enum NotifyDirective {
    One,
    All,
}

impl Waker for LockSignal {
    fn wake(&self) {
        self.signal_all();
    }
}

impl Default for LockSignal {
    fn default() -> Self {
        Self::new()
    }
}

impl LockSignal {
    #[must_use]
    pub fn new() -> Self {
        Self {
            event: CondVar::new(),
            lock: Mutex::new(LockState::Free),
        }
    }

    pub(crate) fn signal(&self, directive: NotifyDirective) {
        let mut state = self.lock.lock().unwrap();
        *state = LockState::Released;
        drop(state);

        // Call notify AFTER releasing the lock
        // This allows waiting threads to immediately acquire the lock
        match directive {
            NotifyDirective::One => self.event.notify_one(),
            NotifyDirective::All => self.event.notify_all(),
        }
    }

    pub fn probe(&self) -> LockState {
        let current_state_guard = self.lock.lock().unwrap();
        let current_state = current_state_guard.clone();
        drop(current_state_guard);
        current_state
    }

    pub fn probe_locked(&self) -> bool {
        let current_state = self.lock.lock().unwrap();
        let is_locked = *current_state == LockState::Locked;
        drop(current_state);
        is_locked
    }

    /// Attempts to put the latch in a locked state thereby
    /// allowing to block the current thread readying
    /// you to call [`Self::wait`] after.
    ///
    /// But the latch might already be in a lock state
    /// and if so, its a no-op and in such cases we return
    /// false to communicate `LockSignal` is already locked
    /// but to be clear this does not mean an error, it just
    /// means something else locked this `LockSignal` and you
    /// are free to call `LockSignal::wait()` as well to wait
    /// for wakeup signal.
    pub fn try_lock(&self) -> bool {
        let mut current_state = self.lock.lock().unwrap();
        if *current_state == LockState::Locked {
            return false;
        }
        *current_state = LockState::Locked;
        drop(current_state);
        true
    }

    /// lock will just perform a lock operation on the
    /// `LockSignal` and internally handle when the lock is
    /// locked as a no-op operation.
    pub fn lock(&self) {
        let mut current_state = self.lock.lock().unwrap();
        if *current_state != LockState::Locked {
            *current_state = LockState::Locked;
        }
        drop(current_state);
    }

    pub fn signal_one(&self) {
        self.signal(NotifyDirective::One);
    }

    pub fn signal_all(&self) {
        self.signal(NotifyDirective::All);
    }

    /// [`lock_and_wait`] will lock the `LockSignal` and
    /// block the current thread till it gets a notification
    /// to wakeup.
    pub fn lock_and_wait(&self) {
        if self.try_lock() {
            tracing::debug!("LockSignal was locked");
        }
        self.wait();
    }

    /// wait blocks the current thread till the signal
    /// is received from the `CondVar`.
    pub fn wait(&self) {
        let mut current_state = self.lock.lock().unwrap();

        // if its in the free state then no wait is required;
        if *current_state == LockState::Free {
            tracing::debug!("Lock is free");
            return;
        }

        // wait till it's released or free
        tracing::debug!("Will loop till lock is free");
        loop {
            if *current_state == LockState::Released {
                // set back to free state
                *current_state = LockState::Free;
                tracing::debug!("Lock is now free");
                return;
            }

            if *current_state == LockState::Free {
                // Another thread already processed the release
                tracing::debug!("Lock already freed by another thread");
                return;
            }

            // wait for the event to be signaled.
            current_state = self.event.wait(current_state).unwrap();
        }
    }
}

#[cfg(test)]
mod test_lock_signals {
    use std::{
        sync::{mpsc, Arc},
        thread,
        time::Duration,
    };

    use crate::synca::{LockState, Waker};

    use super::LockSignal;

    #[test]
    fn can_lock_signal() {
        let latch = Arc::new(LockSignal::new());

        let (sender, receiver) = mpsc::channel::<()>();

        let latch_clone = latch.clone();
        let handler = thread::spawn(move || {
            latch_clone.try_lock();
            sender.send(()).expect("should send");
            latch_clone.wait();
        });

        thread::sleep(Duration::from_millis(100));

        let _ = receiver.recv();
        assert_eq!(LockState::Locked, latch.probe());

        latch.signal_all();
        handler.join().expect("should safely join");

        assert_eq!(LockState::Free, latch.probe());
    }

    #[test]
    fn test_lock_signal_new() {
        let signal = LockSignal::new();
        assert_eq!(LockState::Free, signal.probe());
    }

    #[test]
    fn test_lock_signal_default() {
        let signal = LockSignal::default();
        assert_eq!(LockState::Free, signal.probe());
    }

    #[test]
    fn test_try_lock_success() {
        let signal = LockSignal::new();
        assert!(signal.try_lock());
        assert_eq!(LockState::Locked, signal.probe());
    }

    #[test]
    fn test_try_lock_already_locked() {
        let signal = LockSignal::new();
        assert!(signal.try_lock());
        // Second try_lock should return false since already locked
        assert!(!signal.try_lock());
        assert_eq!(LockState::Locked, signal.probe());
    }

    #[test]
    fn test_lock_operation() {
        let signal = LockSignal::new();
        signal.lock();
        assert_eq!(LockState::Locked, signal.probe());

        // Locking again should be a no-op
        signal.lock();
        assert_eq!(LockState::Locked, signal.probe());
    }

    #[test]
    fn test_probe_locked_true() {
        let signal = LockSignal::new();
        signal.lock();
        assert!(signal.probe_locked());
    }

    #[test]
    fn test_probe_locked_false() {
        let signal = LockSignal::new();
        assert!(!signal.probe_locked());
    }

    #[test]
    fn test_signal_one_releases_lock() {
        let signal = Arc::new(LockSignal::new());
        signal.lock();

        let (tx, rx) = mpsc::channel();
        let signal_clone = signal.clone();
        let handle = thread::spawn(move || {
            // Signal we're about to wait
            tx.send(()).unwrap();
            signal_clone.wait();
        });

        // Wait for thread to be ready
        rx.recv().unwrap();
        thread::sleep(Duration::from_millis(50));

        signal.signal_one();

        handle.join().expect("Thread should complete");
        assert_eq!(LockState::Free, signal.probe());
    }

    #[test]
    fn test_signal_all_releases_lock() {
        let signal = Arc::new(LockSignal::new());
        signal.lock();

        let (tx, rx) = mpsc::channel();
        let signal_clone = signal.clone();
        let handle = thread::spawn(move || {
            // Signal we're about to wait
            tx.send(()).unwrap();
            signal_clone.wait();
        });

        // Wait for thread to be ready
        rx.recv().unwrap();
        thread::sleep(Duration::from_millis(50));

        signal.signal_all();

        handle.join().expect("Thread should complete");
        assert_eq!(LockState::Free, signal.probe());
    }

    #[test]
    fn test_wait_when_free_returns_immediately() {
        let signal = LockSignal::new();
        // Should return immediately without blocking
        signal.wait();
        assert_eq!(LockState::Free, signal.probe());
    }

    #[test]
    fn test_lock_and_wait() {
        let signal = Arc::new(LockSignal::new());

        let (tx, rx) = mpsc::channel();
        let signal_clone = signal.clone();
        let handle = thread::spawn(move || {
            // Signal we're about to lock and wait
            tx.send(()).unwrap();
            signal_clone.lock_and_wait();
        });

        // Wait for thread to be ready
        rx.recv().unwrap();
        thread::sleep(Duration::from_millis(100));

        assert_eq!(LockState::Locked, signal.probe());

        signal.signal_all();
        handle.join().expect("Thread should complete");
        assert_eq!(LockState::Free, signal.probe());
    }

    #[test]
    fn test_multiple_waiters_signal_all() {
        let signal = Arc::new(LockSignal::new());
        signal.lock();

        let mut handles = vec![];

        // Spawn 3 waiting threads
        for _i in 0..3 {
            let signal_clone = signal.clone();
            handles.push(thread::spawn(move || {
                signal_clone.wait();
            }));
        }

        // Give threads time to enter wait state
        thread::sleep(Duration::from_millis(200));

        signal.signal_all();

        // Join all threads - they should all complete
        for handle in handles {
            handle.join().expect("Thread should complete");
        }

        assert_eq!(LockState::Free, signal.probe());
    }

    #[test]
    fn test_waker_trait_implementation() {
        let signal = Arc::new(LockSignal::new());
        signal.lock();

        let (tx, rx) = mpsc::channel();
        let signal_clone = signal.clone();
        let handle = thread::spawn(move || {
            // Signal we're about to wait
            tx.send(()).unwrap();
            signal_clone.wait();
        });

        // Wait for thread to be ready
        rx.recv().unwrap();
        thread::sleep(Duration::from_millis(50));

        // Use Waker trait method
        signal.wake();

        handle.join().expect("Thread should complete");
        assert_eq!(LockState::Free, signal.probe());
    }

    #[test]
    fn test_concurrent_probe_operations() {
        let signal = Arc::new(LockSignal::new());
        signal.lock();

        let mut handles = vec![];
        for _ in 0..5 {
            let signal_clone = signal.clone();
            handles.push(thread::spawn(move || {
                let state = signal_clone.probe();
                assert!(
                    state == LockState::Locked
                        || state == LockState::Released
                        || state == LockState::Free
                );
            }));
        }

        thread::sleep(Duration::from_millis(50));
        signal.signal_all();

        for handle in handles {
            handle.join().expect("Thread should complete");
        }
    }

    #[test]
    fn test_state_transitions() {
        let signal = LockSignal::new();

        // Free -> Locked
        assert_eq!(LockState::Free, signal.probe());
        signal.lock();
        assert_eq!(LockState::Locked, signal.probe());

        // Locked -> Released (via signal) -> Free (after wait completes)
        let signal = Arc::new(signal);
        let (tx, rx) = mpsc::channel();
        let signal_clone = signal.clone();
        let handle = thread::spawn(move || {
            tx.send(()).unwrap();
            signal_clone.wait();
        });

        rx.recv().unwrap();
        thread::sleep(Duration::from_millis(50));
        signal.signal_one();
        handle.join().expect("Thread should complete");

        // Released -> Free (after wait completes)
        assert_eq!(LockState::Free, signal.probe());
    }
}
