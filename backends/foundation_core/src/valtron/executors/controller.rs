use std::time;

pub trait ProcessController {
    /// Signal returned by `yield_for` to indicate whether the caller
    /// should stop processing (e.g. JS `WaitStatus::Waiting`) or
    /// continue (native `()`).
    type YieldSignal;

    /// `yield_for` exits to block the relevant thread efficiently
    /// for the specified period, allowing the thread to be put on
    /// hold by the CPU without spinning uselessly.
    ///
    /// On native: returns `()` — caller ignores it and continues.
    /// On JS: returns `WaitStatus::Waiting` — caller breaks its loop.
    fn yield_for(&self, dur: time::Duration) -> Self::YieldSignal;

    /// Check if the signal returned by `yield_for` indicates the caller
    /// should stop. Default is `false` (native — never stops).
    /// JS impls override to check `WaitStatus::Waiting`.
    fn should_stop(&self, _signal: &Self::YieldSignal) -> bool {
        false
    }
}

pub trait CloneProcessController: ProcessController {
    fn clone_process_controller(&self) -> Box<dyn CloneProcessController<YieldSignal = Self::YieldSignal>>;
}

impl<F> CloneProcessController for F
where
    F: ProcessController + Clone + 'static,
{
    fn clone_process_controller(&self) -> Box<dyn CloneProcessController<YieldSignal = Self::YieldSignal>> {
        Box::new(self.clone())
    }
}
