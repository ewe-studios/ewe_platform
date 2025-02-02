use std::{
    any::Any,
    env,
    sync::{self, Arc},
    thread, time,
};

use concurrent_queue::ConcurrentQueue;
use flume;
use std::str::FromStr;

use crate::synca::{ActivitySignal, DurationStore, Entry, EntryList, LockSignal, OnSignal};

use super::{BoxedLocalExecutionIterator, ProcessController};

#[cfg(not(feature = "web_spin_lock"))]
use std::sync::{Condvar, Mutex};

#[cfg(feature = "web_spin_lock")]
use wasm_sync::{CondVar, Mutex};

#[derive(Default)]
pub struct ThreadYielder {}

impl ProcessController for ThreadYielder {
    fn yield_process(&self) {
        todo!()
    }

    fn yield_for(&self, dur: std::time::Duration) {
        todo!()
    }
}

pub enum ThreadActivity {
    Started(thread::ThreadId),
    Stopped(thread::ThreadId),
    Idle(thread::ThreadId),
    Sleepy(thread::ThreadId),
    Sleeping(thread::ThreadId),
    Paniced(thread::ThreadId, Box<dyn Any + Send>),
}

pub(crate) type SharedThreadRegister = sync::Arc<Mutex<ThreadPoolRegistryInner>>;
pub type SharedTaskQueue = sync::Arc<ConcurrentQueue<BoxedLocalExecutionIterator>>;
pub type SharedActivityQueue = sync::Arc<ConcurrentQueue<ThreadActivity>>;

#[derive(Clone)]
pub(crate) struct Thread {
    /// the queue for tasks
    pub queue: SharedTaskQueue,

    /// the register this thread is related to.
    pub register: SharedThreadRegister,

    /// activity helps indicate the current state of the giving thread.
    pub activity: sync::Arc<ActivitySignal>,

    /// signal used for communicating death to the thread.
    pub kill_signal: sync::Arc<OnSignal>,

    /// global signal used for communicating death to all threads.
    pub global_kill_signal: sync::Arc<OnSignal>,

    /// the unique thread id of the giving thread the thread is associated with.
    pub thread_id: Option<thread::ThreadId>,

    /// the relevant key used by the thread in the core thread registry.
    pub registry_id: Entry,
}

impl Thread {
    pub fn new(
        registry_id: Entry,
        queue: SharedTaskQueue,
        register: SharedThreadRegister,
        global_kill_signal: sync::Arc<OnSignal>,
    ) -> Thread {
        Self {
            queue,
            register,
            registry_id,
            thread_id: None,
            global_kill_signal,
            kill_signal: sync::Arc::new(OnSignal::new()),
            activity: sync::Arc::new(ActivitySignal::new()),
        }
    }
}

pub(crate) struct ThreadPoolRegistryInner {
    threads: EntryList<Thread>,
    sleepers: DurationStore<Entry>,
    latch: Arc<LockSignal>,
    notifications: flume::Sender<ThreadActivity>,
}

impl ThreadPoolRegistryInner {
    pub(crate) fn new(
        notifications: flume::Sender<ThreadActivity>,
        latch: Arc<LockSignal>,
    ) -> SharedThreadRegister {
        sync::Arc::new(Mutex::new(ThreadPoolRegistryInner {
            latch,
            notifications,
            threads: EntryList::new(),
            sleepers: DurationStore::new(),
        }))
    }
}

/// Number of bits used for the thread counters.
#[cfg(target_pointer_width = "64")]
const THREADS_BITS: usize = 16;

#[cfg(target_pointer_width = "32")]
const THREADS_BITS: usize = 8;

/// Max value for the thread counters.
pub(crate) const THREADS_MAX: usize = (1 << THREADS_BITS) - 1;

pub struct ThreadPool {
    num_threads: usize,
    latch: Arc<LockSignal>,
    thread_stack_size: usize,
    op_read_time: time::Duration,
    inner: SharedThreadRegister,
    global_kill_signal: sync::Arc<OnSignal>,
    notifications: flume::Receiver<ThreadActivity>,
}

pub(crate) fn get_num_threads() -> usize {
    match env::var("VALTRON_NUM_THREADS")
        .ok()
        .and_then(|s| usize::from_str(&s).ok())
    {
        Some(x @ 1..) => return x,
        _ => {
            match thread::available_parallelism()
                .ok()
                .and_then(|s| Some(s.get()))
            {
                Some(system_value) => system_value,
                None => 1,
            }
        }
    }
}

impl ThreadPool {
    pub fn new(op_read_time: time::Duration, num_threads: usize, thread_stack_size: usize) -> Self {
        let thread_latch = Arc::new(LockSignal::new());
        let (sender, receiver) = flume::unbounded::<ThreadActivity>();
        Self {
            num_threads,
            op_read_time,
            thread_stack_size,
            notifications: receiver,
            latch: thread_latch.clone(),
            global_kill_signal: sync::Arc::new(OnSignal::new()),
            inner: ThreadPoolRegistryInner::new(sender, thread_latch.clone()),
        }
    }
}

impl ThreadPool {
    pub fn run_until(&self) {
        'mainloop: while !self.global_kill_signal.probe() {
            let thread_activity = match self.notifications.recv_timeout(self.op_read_time) {
                Ok(activity) => activity,
                Err(err) => match err {
                    flume::RecvTimeoutError::Timeout => continue 'mainloop,
                    flume::RecvTimeoutError::Disconnected => break 'mainloop,
                },
            };

            match thread_activity {
                ThreadActivity::Started(thread_id) => todo!(),
                ThreadActivity::Stopped(thread_id) => todo!(),
                ThreadActivity::Idle(thread_id) => todo!(),
                ThreadActivity::Sleepy(thread_id) => todo!(),
                ThreadActivity::Sleeping(thread_id) => todo!(),
                ThreadActivity::Paniced(thread_id, any) => todo!(),
            }
        }
    }
}
