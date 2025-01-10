// Implements Vultron executors based on a multi-threaded model of thread pools that can communicate
// via a ConcurrentQueue that allows different threads in the pool to pull task off the thread at their
// own respective pace.

use std::time;

use crate::valtron::{delayed_iterators, task_iterator};

pub enum State {
    /// Pending indicates the the underlying process to be
    /// still waiting progress to it's next state with
    /// a comunicated indicator of how long possibly that
    /// state might be. Its an optional value that the
    /// underlying process could communicate to the executor
    /// that allows the executor to be smarter about how it
    /// polls for progress.
    Pending(Option<time::Duration>),

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
    pub task: Box<dyn task_iterator::AsTaskIterator<D, P>>,
    pub resolver: Box<dyn task_iterator::resolvers::TaskReadyResolver<D, P>>,
    pub local_mappers: Vec<Box<dyn task_iterator::resolvers::TaskStatusMapper<D, P>>>,
}

impl<D, P> SimpleScheduledTask<D, P> {
    pub fn new(
        iter: Box<dyn task_iterator::AsTaskIterator<D, P>>,
        resolver: Box<dyn task_iterator::resolvers::TaskReadyResolver<D, P>>,
        mappers: Vec<Box<dyn task_iterator::resolvers::TaskStatusMapper<D, P>>>,
    ) -> Self {
        Self {
            resolver,
            task: iter,
            local_mappers: mappers,
        }
    }

    pub fn with_iter(
        iter: Box<dyn task_iterator::AsTaskIterator<D, P>>,
        resolver: Box<dyn task_iterator::resolvers::TaskReadyResolver<D, P>>,
    ) -> Self {
        Self::new(iter, resolver, Vec::new())
    }

    pub fn add_mapper(
        &mut self,
        mapper: Box<dyn task_iterator::resolvers::TaskStatusMapper<D, P>>,
    ) {
        self.local_mappers.push(mapper);
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
    pub task: Box<dyn delayed_iterators::AsDelayedIterator<D>>,
    pub resolver: Box<dyn delayed_iterators::resolvers::DelayedReadyResolver<D>>,
    pub local_mappers: Vec<Box<dyn delayed_iterators::resolvers::DelayedMapper<D>>>,
}

impl<D> SimpleDelayedTask<D> {
    pub fn new(
        iter: Box<dyn delayed_iterators::AsDelayedIterator<D>>,
        resolver: Box<dyn delayed_iterators::resolvers::DelayedReadyResolver<D>>,
        mappers: Vec<Box<dyn delayed_iterators::resolvers::DelayedMapper<D>>>,
    ) -> Self {
        Self {
            resolver,
            task: iter,
            local_mappers: mappers,
        }
    }

    pub fn with_iter(
        iter: Box<dyn delayed_iterators::AsDelayedIterator<D>>,
        resolver: Box<dyn delayed_iterators::resolvers::DelayedReadyResolver<D>>,
    ) -> Self {
        Self::new(iter, resolver, Vec::new())
    }

    pub fn add_mapper(&mut self, mapper: Box<dyn delayed_iterators::resolvers::DelayedMapper<D>>) {
        self.local_mappers.push(mapper);
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
