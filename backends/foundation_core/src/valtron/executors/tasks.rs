// Implements Vultron executors based on a multi-threaded model of thread pools that can communicate
// via a ConcurrentQueue that allows different threads in the pool to pull task off the thread at their
// own respective pace.

use std::marker::PhantomData;

use crate::valtron::{
    task_iterator, BoxedTaskReadyResolver, ExecutionEngine, ExecutionIterator, FnMutReady, FnReady,
    State, TaskStatusMapper,
};

pub struct SimpleScheduledTask<M: ExecutionEngine, D, P = ()> {
    pub task: task_iterator::BoxedTaskIterator<D, P>,
    pub resolver: BoxedTaskReadyResolver<D, P>,
    pub local_mappers: Vec<Box<dyn TaskStatusMapper<D, P>>>,
    _engine: PhantomData<M>,
}

impl<M: ExecutionEngine, D, P> SimpleScheduledTask<M, D, P> {
    pub fn on_next_mut<F, T>(t: T, f: F) -> Self
    where
        T: task_iterator::TaskIterator<Pending = P, Done = D> + 'static,
        F: FnMut(task_iterator::TaskStatus<D, P>, Box<dyn ExecutionEngine>) + 'static,
    {
        let wrapper = FnMutReady::new(f);
        SimpleScheduledTask::new(Box::new(t.into_iter()), Box::new(wrapper), Vec::new())
    }

    pub fn on_next<F, T>(t: T, f: F) -> Self
    where
        T: task_iterator::TaskIterator<Pending = P, Done = D> + 'static,
        F: Fn(task_iterator::TaskStatus<D, P>, Box<dyn ExecutionEngine>) + 'static,
    {
        let wrapper = FnReady::new(f);
        SimpleScheduledTask::new(Box::new(t.into_iter()), Box::new(wrapper), Vec::new())
    }

    pub fn new(
        iter: task_iterator::BoxedTaskIterator<D, P>,
        resolver: BoxedTaskReadyResolver<D, P>,
        mappers: Vec<Box<dyn TaskStatusMapper<D, P>>>,
    ) -> Self {
        Self {
            resolver,
            task: iter,
            local_mappers: mappers,
            _engine: PhantomData::default(),
        }
    }

    pub fn with_iter(
        iter: task_iterator::BoxedTaskIterator<D, P>,
        resolver: BoxedTaskReadyResolver<D, P>,
    ) -> Self {
        Self::new(iter, resolver, Vec::new())
    }

    pub fn add_mapper(mut self, mapper: Box<dyn TaskStatusMapper<D, P>>) -> Self {
        self.local_mappers.push(mapper);
        self
    }
}

impl<M, D, P> ExecutionIterator for SimpleScheduledTask<M, D, P>
where
    M: ExecutionEngine + Into<Box<M>> + 'static,
{
    type Executor = M;

    fn next(&mut self, executor: Self::Executor) -> Option<State> {
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
                                .handle(task_iterator::TaskStatus::Ready(content), executor.into());
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
