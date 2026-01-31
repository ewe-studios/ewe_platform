//! State machine helpers for building TaskIterators.
//!
//! This module provides traits and utilities for implementing TaskIterators
//! using state machine patterns, making it easier to build complex async-like
//! workflows.

use crate::valtron::{ExecutionAction, NoAction, TaskIterator, TaskStatus};
use std::time::Duration;

/// Result of a state machine transition.
///
/// WHY: Encapsulates all possible state machine outcomes in one type
/// WHAT: Enum representing Continue, Yield, Complete, Error, Delay, or Spawn
pub enum StateTransition<S, O, E, A = NoAction>
where
    A: ExecutionAction,
{
    /// Continue processing in new state (non-blocking)
    Continue(S),

    /// Yield a value and transition to new state
    Yield(O, S),

    /// Task complete with final value
    Complete(O),

    /// Task failed with error
    Error(E),

    /// Delay before continuing (for retries, backoff)
    Delay(Duration, S),

    /// Spawn a child task and continue
    Spawn(A, S),
}

/// Trait for implementing state machine logic.
///
/// WHY: Provides clean abstraction for state-based TaskIterator implementation
/// WHAT: Define State, Output, Error types and transition() logic
pub trait StateMachine {
    /// The state type for this machine
    type State: Clone;

    /// The output type produced by this machine
    type Output;

    /// The error type for failures
    type Error;

    /// The action type for spawning (use NoAction if not spawning)
    type Action: ExecutionAction;

    /// Perform one transition from the current state.
    ///
    /// WHY: Core state machine logic - defines behavior at each state
    /// WHAT: Given current state, return next transition
    fn transition(
        &mut self,
        state: Self::State,
    ) -> StateTransition<Self::State, Self::Output, Self::Error, Self::Action>;

    /// Get the initial state.
    ///
    /// WHY: State machines need a starting point
    /// WHAT: Returns the first state for the machine
    fn initial_state(&self) -> Self::State;
}

/// Wrapper that implements TaskIterator for any StateMachine.
///
/// WHY: Adapts StateMachine trait to TaskIterator trait
/// WHAT: Drives state machine by calling transition() on each next()
pub struct StateMachineTask<M: StateMachine> {
    machine: M,
    current_state: Option<M::State>,
}

impl<M: StateMachine> StateMachineTask<M> {
    pub fn new(machine: M) -> Self {
        let initial = machine.initial_state();
        Self {
            machine,
            current_state: Some(initial),
        }
    }
}

impl<M> TaskIterator for StateMachineTask<M>
where
    M: StateMachine,
{
    type Ready = M::Output;
    type Pending = M::State;
    type Spawner = M::Action;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let state = self.current_state.take()?;

        match self.machine.transition(state) {
            StateTransition::Continue(next) => {
                self.current_state = Some(next.clone());
                Some(TaskStatus::Pending(next))
            }
            StateTransition::Yield(output, next) => {
                self.current_state = Some(next);
                Some(TaskStatus::Ready(output))
            }
            StateTransition::Complete(output) => {
                // State machine complete, no more states
                Some(TaskStatus::Ready(output))
            }
            StateTransition::Error(_err) => {
                // Map error to None (task failed and stops)
                // Design decision: StateMachines handle errors internally
                // or propagate via Output type (e.g., Result<T, E>)
                tracing::warn!(
                    "State machine error (ignored): {:?}",
                    std::any::type_name::<M::Error>()
                );
                None
            }
            StateTransition::Delay(duration, next) => {
                self.current_state = Some(next);
                Some(TaskStatus::Delayed(duration))
            }
            StateTransition::Spawn(action, next) => {
                self.current_state = Some(next);
                Some(TaskStatus::Spawn(action))
            }
        }
    }
}

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    // Test StateMachine: Counter that yields numbers from 0 to N
    #[derive(Clone, Debug, PartialEq)]
    enum CounterState {
        Counting(i32),
        Done,
    }

    struct CounterMachine {
        max: i32,
    }

    impl StateMachine for CounterMachine {
        type State = CounterState;
        type Output = i32;
        type Error = ();
        type Action = NoAction;

        fn transition(
            &mut self,
            state: Self::State,
        ) -> StateTransition<Self::State, Self::Output, Self::Error, Self::Action> {
            match state {
                CounterState::Counting(n) if n < self.max => {
                    StateTransition::Yield(n, CounterState::Counting(n + 1))
                }
                CounterState::Counting(n) => StateTransition::Complete(n),
                CounterState::Done => StateTransition::Complete(-1),
            }
        }

        fn initial_state(&self) -> Self::State {
            CounterState::Counting(0)
        }
    }

    /// WHY: StateMachineTask must drive state machine through transitions
    /// WHAT: Counter machine yields 0, 1, 2 then completes
    #[test]
    fn test_state_machine_task_yields_values() {
        let machine = CounterMachine { max: 3 };
        let mut task = StateMachineTask::new(machine);

        // Should yield 0
        match task.next() {
            Some(TaskStatus::Ready(0)) => {}
            other => panic!("Expected Ready(0), got {:?}", other),
        }

        // Should yield 1
        match task.next() {
            Some(TaskStatus::Ready(1)) => {}
            other => panic!("Expected Ready(1), got {:?}", other),
        }

        // Should yield 2
        match task.next() {
            Some(TaskStatus::Ready(2)) => {}
            other => panic!("Expected Ready(2), got {:?}", other),
        }

        // Should complete with 3
        match task.next() {
            Some(TaskStatus::Ready(3)) => {}
            other => panic!("Expected Ready(3), got {:?}", other),
        }

        // Done
        assert!(task.next().is_none());
    }

    /// WHY: StateTransition::Continue must allow non-yielding state changes
    /// WHAT: Machine continues without yielding output
    #[test]
    fn test_state_transition_continue() {
        #[allow(dead_code)]
        #[derive(Clone, Debug, PartialEq)]
        enum State {
            Init,
            Processing,
            Done,
        }

        struct ContinueMachine {
            steps: Arc<StdMutex<Vec<State>>>,
        }

        impl StateMachine for ContinueMachine {
            type State = State;
            type Output = String;
            type Error = ();
            type Action = NoAction;

            fn transition(
                &mut self,
                state: Self::State,
            ) -> StateTransition<Self::State, Self::Output, Self::Error, Self::Action> {
                self.steps.lock().unwrap().push(state.clone());
                match state {
                    State::Init => StateTransition::Continue(State::Processing),
                    State::Processing => StateTransition::Complete("done".to_string()),
                    State::Done => StateTransition::Complete("already done".to_string()),
                }
            }

            fn initial_state(&self) -> Self::State {
                State::Init
            }
        }

        let steps = Arc::new(StdMutex::new(Vec::new()));
        let machine = ContinueMachine {
            steps: steps.clone(),
        };
        let mut task = StateMachineTask::new(machine);

        // First next() should be Pending(Init) after Continue
        match task.next() {
            Some(TaskStatus::Pending(State::Processing)) => {}
            other => panic!("Expected Pending(done), got {other:?}"),
        }

        // Next should complete
        match task.next() {
            Some(TaskStatus::Ready(ref s)) if s == "done" => {}
            other => panic!("Expected Ready(done), got {other:?}"),
        }

        // Verify states were visited
        let visited = steps.lock().unwrap();
        assert_eq!(*visited, vec![State::Init, State::Processing]);
    }

    /// WHY: StateTransition::Delay must emit TaskStatus::Delayed
    /// WHAT: Machine can request delayed continuation
    #[test]
    fn test_state_transition_delay() {
        #[allow(dead_code)]
        #[derive(Clone, Debug, PartialEq)]
        enum State {
            Start,
            Delayed,
            End,
        }

        struct DelayMachine;

        impl StateMachine for DelayMachine {
            type State = State;
            type Output = String;
            type Error = ();
            type Action = NoAction;

            fn transition(
                &mut self,
                state: Self::State,
            ) -> StateTransition<Self::State, Self::Output, Self::Error, Self::Action> {
                match state {
                    State::Start => {
                        StateTransition::Delay(Duration::from_millis(100), State::Delayed)
                    }
                    State::Delayed => StateTransition::Complete("after delay".to_string()),
                    State::End => StateTransition::Complete("done".to_string()),
                }
            }

            fn initial_state(&self) -> Self::State {
                State::Start
            }
        }

        let machine = DelayMachine;
        let mut task = StateMachineTask::new(machine);

        // Should emit Delayed
        match task.next() {
            Some(TaskStatus::Delayed(d)) if d == Duration::from_millis(100) => {}
            other => panic!("Expected Delayed(100ms), got {:?}", other),
        }

        // After delay, should complete
        match task.next() {
            Some(TaskStatus::Ready(ref s)) if s == "after delay" => {}
            other => panic!("Expected Ready(after delay), got {:?}", other),
        }
    }

    /// WHY: StateTransition::Error must stop the task
    /// WHAT: Error transitions return None from next()
    #[test]
    fn test_state_transition_error_stops_task() {
        #[allow(dead_code)]
        #[derive(Clone, Debug, PartialEq)]
        enum State {
            Start,
            Error,
        }

        struct ErrorMachine;

        impl StateMachine for ErrorMachine {
            type State = State;
            type Output = String;
            type Error = String;
            type Action = NoAction;

            fn transition(
                &mut self,
                state: Self::State,
            ) -> StateTransition<Self::State, Self::Output, Self::Error, Self::Action> {
                match state {
                    State::Start => StateTransition::Error("something went wrong".to_string()),
                    State::Error => StateTransition::Complete("unreachable".to_string()),
                }
            }

            fn initial_state(&self) -> Self::State {
                State::Start
            }
        }

        let machine = ErrorMachine;
        let mut task = StateMachineTask::new(machine);

        // Error should stop the task
        assert!(task.next().is_none());
    }

    /// WHY: StateTransition::Spawn must emit TaskStatus::Spawn
    /// WHAT: Machine can spawn child tasks
    #[test]
    fn test_state_transition_spawn() {
        use crate::valtron::executors::SpawnWithSchedule;

        #[allow(dead_code)]
        #[derive(Clone, Debug, PartialEq)]
        enum State {
            Start,
            Spawned,
            End,
        }

        struct SpawnMachine;

        impl StateMachine for SpawnMachine {
            type State = State;
            type Output = String;
            type Error = ();
            type Action = SpawnWithSchedule<Box<dyn FnOnce() + Send>>;

            fn transition(
                &mut self,
                state: Self::State,
            ) -> StateTransition<Self::State, Self::Output, Self::Error, Self::Action> {
                match state {
                    State::Start => {
                        let action =
                            SpawnWithSchedule::new(Box::new(|| {}) as Box<dyn FnOnce() + Send>);
                        StateTransition::Spawn(action, State::Spawned)
                    }
                    State::Spawned => StateTransition::Complete("spawned task".to_string()),
                    State::End => StateTransition::Complete("done".to_string()),
                }
            }

            fn initial_state(&self) -> Self::State {
                State::Start
            }
        }

        let machine = SpawnMachine;
        let mut task = StateMachineTask::new(machine);

        // Should emit Spawn
        match task.next() {
            Some(TaskStatus::Spawn(_)) => {}
            other => panic!("Expected Spawn, got {:?}", other),
        }

        // After spawn, should complete
        match task.next() {
            Some(TaskStatus::Ready(ref s)) if s == "spawned task" => {}
            other => panic!("Expected Ready(spawned task), got {:?}", other),
        }
    }
}
