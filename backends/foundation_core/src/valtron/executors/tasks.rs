// Implements Vultron executors based on a multi-threaded model of thread pools that can communicate
// via a ConcurrentQueue that allows different threads in the pool to pull task off the thread at their
// own respective pace.

use std::marker::PhantomData;

use crate::valtron::{task_iterator, ExecutionEngine, ExecutionIterator};

pub struct SimpleScheduledTask<E, X, D, P = ()>
where
    E: ExecutionIterator,
    X: ExecutionEngine<E>,
{
    pub task: task_iterator::BoxedTaskIterator<D, P>,
    pub resolver: task_iterator::BoxedTaskReadyResolver<D, P, E, X>,
    pub local_mappers: Vec<Box<dyn task_iterator::resolvers::TaskStatusMapper<D, P>>>,
}

impl<E, X, D, P> SimpleScheduledTask<E, X, D, P>
where
    E: ExecutionIterator,
    X: ExecutionEngine<E>,
{
    pub fn on_next_mut<F, T>(t: T, f: F) -> Self
    where
        T: task_iterator::TaskIterator<Pending = P, Done = D> + 'static,
        F: FnMut(task_iterator::TaskStatus<D, P>, X) + 'static,
    {
        let wrapper = task_iterator::resolvers::FnMutReady::new(f);
        SimpleScheduledTask::new(Box::new(t.into_iter()), Box::new(wrapper), Vec::new())
    }

    pub fn on_next<F, T>(t: T, f: F) -> Self
    where
        T: task_iterator::TaskIterator<Pending = P, Done = D> + 'static,
        F: Fn(task_iterator::TaskStatus<D, P>, X) + 'static,
    {
        let wrapper = task_iterator::resolvers::FnReady::new(f);
        SimpleScheduledTask::new(Box::new(t.into_iter()), Box::new(wrapper), Vec::new())
    }

    pub fn new(
        iter: task_iterator::BoxedTaskIterator<D, P>,
        resolver: Box<dyn task_iterator::TaskReadyResolver<D, P, E, X>>,
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
        resolver: Box<dyn task_iterator::TaskReadyResolver<D, P, E, X>>,
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

// impl<E, D, P> ExecutionIterator for SimpleScheduledTask<E, D, P> {
//     fn next(
//         &self,
//         executor: impl crate::valtron::ExecutionEngine,
//     ) -> Option<crate::valtron::State> {
//         todo!()
//     }

//     fn next(&mut self, item: impl ExecutionEngine) -> Option<Self::Item> {
//         Some(match self.task.next() {
//             Some(inner) => {
//                 let mut previous_response = Some(inner);
//                 for mapper in &mut self.local_mappers {
//                     previous_response = mapper.map(previous_response);
//                 }
//                 match previous_response {
//                     Some(value) => match value {
//                         task_iterator::TaskStatus::Delayed(dur) => State::Pending(Some(dur)),
//                         task_iterator::TaskStatus::Pending(_) => State::Pending(None),
//                         task_iterator::TaskStatus::Init => State::Pending(None),
//                         task_iterator::TaskStatus::Ready(content) => {
//                             self.resolver
//                                 .handle(task_iterator::TaskStatus::Ready(content));
//                             State::Progressed
//                         }
//                     },
//                     None => State::Pending(None),
//                 }
//             }
//             None => State::Done,
//         })
//     }
// }
