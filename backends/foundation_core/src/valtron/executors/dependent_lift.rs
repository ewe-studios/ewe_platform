use crate::synca::Entry;
use crate::valtron::{
    BoxedExecutionEngine, BoxedExecutionIterator, ExecutionIterator, SpawnInfo, State,
};

#[allow(dead_code)]
pub(crate) struct LinkedParentChildTaskInner {
    pub info: SpawnInfo,
    pub parent: Option<BoxedExecutionIterator>,
    pub child: Option<BoxedExecutionIterator>,
    /// Stores parent state signals that require executor action when child is active.
    /// See Feature 02: Linked Task State Propagation for details.
    pub pending_parent_state: Option<State>,
}

/// `DependentLiftedTask` defines a linked task where a parent [`ExecutionIterator`]
/// with a child [`ExecutionIterator`] where an execution of the child [`ExecutionIterator::next`]
/// method requires a execution of the parent's [`ExecutionIterator::next`].
///
/// This allows us created an interlinked sequential process where progress in one means progress
/// in the child will cause progress in the parent, returning the state of the child ignoring that
/// of the parent until the child is exhausted, leaving only the parent to continue operating.
///
pub struct DualSequeunceChildAndParentLinkedTask(LinkedParentChildTaskInner);

impl DualSequeunceChildAndParentLinkedTask {
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
            pending_parent_state: None,
        })
    }
}

impl ExecutionIterator for DualSequeunceChildAndParentLinkedTask {
    fn next(&mut self, parent_id: Entry, engine: BoxedExecutionEngine) -> Option<State> {
        // Feature 02: Check for pending parent state first
        // If the parent previously returned a state requiring executor action
        // (Pending with duration, SpawnFinished, Panicked, Reschedule), we
        // stored it and must return it before polling the child again.
        if let Some(pending) = self.0.pending_parent_state.take() {
            return Some(pending);
        }

        if let Some(mut child) = self.0.child.take() {
            if let Some(child_state) = child.next(parent_id, engine.boxed_engine()) {
                if child_state != State::Done {
                    self.0.child = Some(child);

                    // Poll the parent but capture its actual state
                    if let Some(mut parent) = self.0.parent.take() {
                        if let Some(parent_state) = parent.next(parent_id, engine) {
                            // Feature 02: Check if parent state requires executor action
                            // These states need to be propagated to the executor even
                            // when the child is active
                            let needs_propagation = matches!(
                                parent_state,
                                State::Pending(Some(_))
                                    | State::SpawnFinished(_)
                                    | State::Panicked
                                    | State::Reschedule
                            );

                            // Store parent state before we move it
                            let parent_is_done = parent_state == State::Done;

                            if needs_propagation {
                                // Store the parent state to return on next call
                                self.0.pending_parent_state = Some(parent_state);
                            }

                            // Parent is still active (not Done), restore it
                            if !parent_is_done {
                                self.0.parent = Some(parent);
                            }
                        }
                    }

                    return Some(child_state);
                }
            }
        }

        // Child is exhausted (Done or None), continue with parent only
        if let Some(mut parent) = self.0.parent.take() {
            if let Some(parent_state) = parent.next(parent_id, engine) {
                if parent_state != State::Done {
                    self.0.parent = Some(parent);
                }
                return Some(parent_state);
            }
        }

        // Both child and parent are exhausted
        None
    }
}

/// `FinishLiftedBeforeLifterTask` defines a linked task where a parent [`ExecutionIterator`]
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
pub struct FinishChildBeforeParentTask(LinkedParentChildTaskInner);

impl FinishChildBeforeParentTask {
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
            pending_parent_state: None,
        })
    }
}

impl ExecutionIterator for FinishChildBeforeParentTask {
    fn next(&mut self, parent_id: Entry, engine: BoxedExecutionEngine) -> Option<State> {
        if let Some(mut child) = self.0.child.take() {
            if let Some(child_state) = child.next(parent_id, engine.boxed_engine()) {
                if child_state != State::Done {
                    self.0.child = Some(child);
                    return Some(child_state);
                }
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
