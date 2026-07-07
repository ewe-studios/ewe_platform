//! Shared `InlineActionBehaviour` enum — no trait bounds, so it compiles under
//! both `multi` and `not(multi)`. The dispatch logic lives in the per-build
//! `InlineAction` impls (sendables / non_sendables).

/// Which verb a [`super::InlineAction`] should use when its [`ExecutionAction`]
/// fires. The `Broadcast` variant is only valid under `multi`; under
/// `not(multi)` it is unreachable.
#[derive(Clone, Copy, Default)]
pub enum InlineActionBehaviour {
    /// Sequence the task with a consuming iterator (needs a parent `Entry`).
    #[default]
    Sequenced,
    /// Lift the task onto the local queue (optional parent).
    Lift,
    /// Lift the task with an explicit parent.
    LiftWithParent,
    /// Schedule the task at the bottom of the local queue.
    Schedule,
    /// Broadcast the task to the global queue (only under `feature = "multi"`).
    Broadcast,
}
