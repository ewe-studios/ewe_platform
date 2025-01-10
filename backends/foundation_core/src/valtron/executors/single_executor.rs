use std::{cell, rc, sync};

use crate::synca::{Entry, EntryList, Sleepers, Waker};

use super::State;
use concurrent_queue::ConcurrentQueue;

struct SingleWaker {
    task_handle: Entry,
    executor: rc::Rc<cell::RefCell<SingleExecutorState>>,
}

impl Waker for SingleWaker {
    fn wake(&self) {
        todo!()
    }
}

pub type BoxedStateIterator = Box<dyn Iterator<Item = State>>;

pub(crate) struct SingleExecutorState {
    global_tasks: sync::Arc<ConcurrentQueue<BoxedStateIterator>>,
    local_tasks: EntryList<BoxedStateIterator>,
    idle_tasks: EntryList<BoxedStateIterator>,
    active_tasks: EntryList<BoxedStateIterator>,
    sleepers: Sleepers<SingleWaker>,
}

pub struct SingleExecutor {
    state: rc::Rc<cell::RefCell<SingleExecutorState>>,
}
