use std::{any::Any, marker::PhantomData};

use crate::synca::Entry;
use crate::valtron::{
    BoxedExecutionEngine, BoxedExecutionIterator, BoxedPanicHandler, BoxedSendExecutionIterator,
    ExecutionAction, ExecutionIterator, SpawnInfo, State, TaskIterator, TaskStatus,
};

pub(crate) struct DependentLiftedTaskInner {
    pub info: SpawnInfo,
    pub parent: Option<BoxedExecutionIterator>,
    pub child: Option<BoxedExecutionIterator>,
}

/// [`DependentLiftedTask`] defines a linked task where a parent [`ExecutionIterator`]
/// with a child [`ExecutionIterator`] where an execution of the child [`ExecutionIterator::next`]
/// method requires a execution of the parent's [`ExecutionIterator::next`].
///
/// This allows us created an interlinked sequential process where progress in one means progress
/// in the
///
pub struct DependentLiftedTask(DependentLiftedTaskInner);

impl DependentLiftedTask {
    #[must_use]
    pub fn new(
        info: SpawnInfo,
        parent: BoxedExecutionIterator,
        child: BoxedExecutionIterator,
    ) -> Self {
        Self(DependentLiftedTaskInner {
            info,
            parent: Some(parent),
            child: Some(child),
        })
    }
}

impl ExecutionIterator for DependentLiftedTask {
    fn next(&mut self, parent_id: Entry, engine: BoxedExecutionEngine) -> Option<State> {
        if let Some(mut child) = self.0.child.take() {
            if let Some(child_state) = child.next(parent_id, engine.boxed_engine()) {
                self.0.child = Some(child);

                // get the parent and also perform next
                if let Some(mut parent) = self.0.parent.take() {
                    // if the parent outputs Some then reset the parent
                    if parent.next(parent_id, engine).is_some() {
                        self.0.parent = Some(parent);
                    }
                }

                return Some(child_state);
            }
        }

        // get the parent has child is now None and make progress with parent only
        // until parent returns None.
        if let Some(mut parent) = self.0.parent.take() {
            // if the parent outputs Some then reset the parent
            if let Some(parent_state) = parent.next(parent_id, engine) {
                self.0.parent = Some(parent);
                return Some(parent_state);
            }
        }

        // child and parent are no more active return None
        None
    }
}
