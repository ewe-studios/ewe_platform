// Implements Vultron executors based on a multi-threaded model of thread pools that can communicate
// via a ConcurrentQueue that allows different threads in the pool to pull task off the thread at their
// own respective pace.

use std::marker::PhantomData;

use super::{
    BoxedTaskIterator, BoxedTaskReadyResolver, ExecutionAction, ExecutionEngine, ExecutionIterator,
    FnMutReady, FnReady, LocalExecutorEngine, State, TaskIterator, TaskStatus, TaskStatusMapper,
};
use crate::synca::Entry;

pub struct SimpleScheduledTask<M, S: ExecutionAction, D, P = ()> {
    pub task: BoxedTaskIterator<D, P, S>,
    pub resolver: BoxedTaskReadyResolver<M, S, D, P>,
    pub local_mappers: Vec<Box<dyn TaskStatusMapper<D, P, S>>>,
    _engine: PhantomData<M>,
}

impl<M, S, D, P> SimpleScheduledTask<M, S, D, P>
where
    S: ExecutionAction + 'static,
{
    pub fn new(
        iter: BoxedTaskIterator<D, P, S>,
        resolver: BoxedTaskReadyResolver<M, S, D, P>,
        mappers: Vec<Box<dyn TaskStatusMapper<D, P, S>>>,
    ) -> Self {
        Self {
            resolver,
            task: iter,
            local_mappers: mappers,
            _engine: PhantomData::default(),
        }
    }

    pub fn on_next_mut<F, T>(t: T, f: F) -> Self
    where
        M: ExecutionEngine<Executor = LocalExecutorEngine> + 'static,
        T: TaskIterator<Pending = P, Done = D, Spawner = S> + 'static,
        F: FnMut(TaskStatus<D, P, S>, M) + 'static,
    {
        let wrapper = FnMutReady::new(f);
        SimpleScheduledTask::new(Box::new(t.into_iter()), Box::new(wrapper), Vec::new())
    }

    pub fn on_next<F, T>(t: T, f: F) -> Self
    where
        M: ExecutionEngine<Executor = LocalExecutorEngine> + 'static,
        T: TaskIterator<Pending = P, Done = D, Spawner = S> + 'static,
        F: Fn(TaskStatus<D, P, S>, M) + 'static,
    {
        let wrapper = FnReady::new(f);
        SimpleScheduledTask::new(Box::new(t.into_iter()), Box::new(wrapper), Vec::new())
    }

    pub fn with_iter(
        iter: BoxedTaskIterator<D, P, S>,
        resolver: BoxedTaskReadyResolver<M, S, D, P>,
    ) -> Self {
        Self::new(iter, resolver, Vec::new())
    }

    pub fn add_mapper(mut self, mapper: Box<dyn TaskStatusMapper<D, P, S>>) -> Self {
        self.local_mappers.push(mapper);
        self
    }
}

impl<D, P, S> ExecutionIterator for SimpleScheduledTask<LocalExecutorEngine, S, D, P>
where
    S: ExecutionAction<Engine = LocalExecutorEngine>,
{
    type Executor = LocalExecutorEngine;

    fn next(&mut self, entry: Entry, executor: Self::Executor) -> Option<State> {
        Some(match self.task.next() {
            Some(inner) => {
                let mut previous_response = Some(inner);
                for mapper in &mut self.local_mappers {
                    previous_response = mapper.map(previous_response);
                }
                match previous_response {
                    Some(value) => match value {
                        TaskStatus::Delayed(dur) => State::Pending(Some(dur)),
                        TaskStatus::Pending(_) => State::Pending(None),
                        TaskStatus::Init => State::Pending(None),
                        TaskStatus::Spawn(action) => match action.apply(entry, executor) {
                            Ok(_) => State::Progressed,
                            Err(err) => {
                                tracing::error!("Failed to apply ExectionAction: {:?}", err);
                                State::SpawnFailed
                            }
                        },
                        TaskStatus::Ready(content) => {
                            self.resolver.handle(TaskStatus::Ready(content), executor);
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
