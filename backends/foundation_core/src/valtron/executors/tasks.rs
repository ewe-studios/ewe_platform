// Implements Vultron executors based on a multi-threaded model of thread pools that can communicate
// via a ConcurrentQueue that allows different threads in the pool to pull task off the thread at their
// own respective pace.

use std::time;

use crate::valtron::{delayed_iterators, task_iterator};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// Pending indicates the the underlying process to be
    /// still waiting progress to it's next state with
    /// a comunicated indicator of how long possibly that
    /// state might be. Its an optional value that the
    /// underlying process could communicate to the executor
    /// that allows the executor to be smarter about how it
    /// polls for progress.
    Pending(Option<time::Duration>),

    /// Reschedule indicates we want to rechedule the underlying
    /// task leaving the performance of that to the underlying
    /// process that receives this.
    Reschedule,

    /// Progressed simply indicates the underlying iterator
    /// has progressed in it's state. This lets the executor
    /// perform whatever tracking/progress logic it needs to do
    /// in relation to this.
    Progressed,

    /// Done indicates that the iterator has finished (when it returns None)
    /// and no further execution is required for giving iterator.
    Done,
}

pub struct SimpleScheduledTask<D, P = ()> {
    pub task: task_iterator::BoxedTaskIterator<D, P>,
    pub resolver: task_iterator::BoxedTaskReadyResolver<D, P>,
    pub local_mappers: Vec<Box<dyn task_iterator::resolvers::TaskStatusMapper<D, P>>>,
}

impl<D, P> SimpleScheduledTask<D, P> {
    pub fn on_next_mut<F, T>(t: T, f: F) -> Self
    where
        T: task_iterator::TaskIterator<Pending = P, Done = D> + 'static,
        F: FnMut(task_iterator::TaskStatus<D, P>) + 'static,
    {
        let wrapper = task_iterator::resolvers::FnMutReady::new(f);
        SimpleScheduledTask::new(Box::new(t.into_iter()), Box::new(wrapper), Vec::new())
    }

    pub fn on_next<F, T>(t: T, f: F) -> Self
    where
        T: task_iterator::TaskIterator<Pending = P, Done = D> + 'static,
        F: Fn(task_iterator::TaskStatus<D, P>) + 'static,
    {
        let wrapper = task_iterator::resolvers::FnReady::new(f);
        SimpleScheduledTask::new(Box::new(t.into_iter()), Box::new(wrapper), Vec::new())
    }

    pub fn new(
        iter: task_iterator::BoxedTaskIterator<D, P>,
        resolver: Box<dyn task_iterator::TaskReadyResolver<D, P>>,
        mappers: Vec<Box<dyn task_iterator::resolvers::TaskStatusMapper<D, P>>>,
    ) -> Self {
        Self {
            resolver,
            task: iter,
            local_mappers: mappers,
        }
    }

    pub fn with_iter(
        iter: task_iterator::BoxedTaskIterator<D, P>,
        resolver: Box<dyn task_iterator::TaskReadyResolver<D, P>>,
    ) -> Self {
        Self::new(iter, resolver, Vec::new())
    }

    pub fn add_mapper(
        mut self,
        mapper: Box<dyn task_iterator::resolvers::TaskStatusMapper<D, P>>,
    ) -> Self {
        self.local_mappers.push(mapper);
        self
    }
}

impl<D, P> Iterator for SimpleScheduledTask<D, P> {
    type Item = State;

    fn next(&mut self) -> Option<Self::Item> {
        Some(match self.task.next() {
            Some(inner) => {
                let mut previous_response = Some(inner);
                for mapper in &mut self.local_mappers {
                    previous_response = mapper.map(previous_response);
                }
                match previous_response {
                    Some(value) => match value {
                        task_iterator::TaskStatus::Delayed(dur) => State::Pending(Some(dur)),
                        task_iterator::TaskStatus::Pending(_) => State::Pending(None),
                        task_iterator::TaskStatus::Init => State::Pending(None),
                        task_iterator::TaskStatus::Ready(content) => {
                            self.resolver
                                .handle(task_iterator::TaskStatus::Ready(content));
                            State::Progressed
                        }
                    },
                    None => State::Pending(None),
                }
            }
            None => State::Done,
        })
    }
}

pub struct SimpleDelayedTask<D> {
    pub task: delayed_iterators::BoxedDelayedIterator<D>,
    pub resolver: Box<dyn delayed_iterators::DelayedReadyResolver<D>>,
    pub local_mappers: Vec<Box<dyn delayed_iterators::resolvers::DelayedMapper<D>>>,
}

impl<D> SimpleDelayedTask<D> {
    pub fn on_next<F, T>(t: T, f: F) -> Self
    where
        T: delayed_iterators::DelayedIterator<Item = D> + 'static,
        F: Fn(delayed_iterators::Delayed<D>) + 'static,
    {
        let wrapper = delayed_iterators::resolvers::DelayedFnReady::new(f);
        Self::new(Box::new(t.into_iter()), Box::new(wrapper), Vec::new())
    }

    pub fn new(
        iter: delayed_iterators::BoxedDelayedIterator<D>,
        resolver: Box<dyn delayed_iterators::DelayedReadyResolver<D>>,
        mappers: Vec<Box<dyn delayed_iterators::resolvers::DelayedMapper<D>>>,
    ) -> Self {
        Self {
            resolver,
            task: iter,
            local_mappers: mappers,
        }
    }

    pub fn with_iter(
        iter: delayed_iterators::BoxedDelayedIterator<D>,
        resolver: Box<dyn delayed_iterators::DelayedReadyResolver<D>>,
    ) -> Self {
        Self::new(iter, resolver, Vec::new())
    }

    pub fn add_mapper(
        mut self,
        mapper: Box<dyn delayed_iterators::resolvers::DelayedMapper<D>>,
    ) -> Self {
        self.local_mappers.push(mapper);
        self
    }
}

impl<D> Iterator for SimpleDelayedTask<D> {
    type Item = State;

    fn next(&mut self) -> Option<Self::Item> {
        Some(match self.task.next() {
            Some(inner) => {
                let mut previous_response: Option<delayed_iterators::Delayed<D>> = Some(inner);
                for mapper in &mut self.local_mappers {
                    previous_response = mapper.map(previous_response);
                }
                match previous_response {
                    Some(value) => match value {
                        delayed_iterators::Delayed::Pending(_from, _within, rem) => {
                            State::Pending(Some(rem))
                        }
                        delayed_iterators::Delayed::Done(content) => {
                            self.resolver
                                .handle(delayed_iterators::Delayed::Done(content));
                            State::Progressed
                        }
                    },
                    None => State::Pending(None),
                }
            }
            None => State::Done,
        })
    }
}
