use crate::synca::Entry;
use crate::valtron::{
    BoxedExecutionEngine, BoxedExecutionIterator, ExecutionIterator, SpawnInfo, State, TaskIterator,
};

#[allow(dead_code)]
pub(crate) struct LinkedParentChildTaskInner {
    pub info: SpawnInfo,
    pub parent: Option<BoxedExecutionIterator>,
    pub child: Option<BoxedExecutionIterator>,
}

/// [`DependentLiftedTask`] defines a linked task where a parent [`ExecutionIterator`]
/// with a child [`ExecutionIterator`] where an execution of the child [`ExecutionIterator::next`]
/// method requires a execution of the parent's [`ExecutionIterator::next`].
///
/// This allows us created an interlinked sequential process where progress in one means progress
/// in the child will cause progress in the parent, returning the state of the child ignoring that
/// of the parent until the child is exhausted, leaving only the parent to continue operating.
///
pub struct DualSequeunceLiftedLinkedTask(LinkedParentChildTaskInner);

impl DualSequeunceLiftedLinkedTask {
    #[must_use]
    pub fn new(
        info: SpawnInfo,
        parent: BoxedExecutionIterator,
        child: BoxedExecutionIterator,
    ) -> Self {
        Self(LinkedParentChildTaskInner {
            info,
            parent: Some(parent),
            child: Some(child),
        })
    }
}

impl ExecutionIterator for DualSequeunceLiftedLinkedTask {
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

/// [`FinishLiftedBeforeLifterTask`] defines a linked task where a parent [`ExecutionIterator`]
/// with a child [`ExecutionIterator`] are mutually bound to each other but child task must
/// first finish before the parent will continue to make any progress.
///
/// This means we have scenario where an execution of the child [`ExecutionIterator::next`]
/// must be called until it returns None (no more values) before execution
/// of the parent's [`ExecutionIterator::next`].
///
/// This allows us created an interlinked sequential process we encapsulate this relationship
/// into a new task type that will own this too and reduce coordination at a larger level.
///
pub struct FinishLiftedBeforeLifterTask(LinkedParentChildTaskInner);

impl FinishLiftedBeforeLifterTask {
    #[must_use]
    pub fn new(
        info: SpawnInfo,
        parent: BoxedExecutionIterator,
        child: BoxedExecutionIterator,
    ) -> Self {
        Self(LinkedParentChildTaskInner {
            info,
            parent: Some(parent),
            child: Some(child),
        })
    }
}

impl ExecutionIterator for FinishLiftedBeforeLifterTask {
    fn next(&mut self, parent_id: Entry, engine: BoxedExecutionEngine) -> Option<State> {
        if let Some(mut child) = self.0.child.take() {
            if let Some(child_state) = child.next(parent_id, engine.boxed_engine()) {
                self.0.child = Some(child);
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
