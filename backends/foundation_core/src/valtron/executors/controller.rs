use std::time;

pub trait ProcessController {
    /// `yield_for` specifically exits to block the relevant
    /// thread efficiently for the specified period of time which
    /// will allow the giving thread to be put on old by the CPU
    /// without spining uselessly, but this also depends on
    /// the platform and what support it might have.
    fn yield_for(&self, dur: time::Duration);
}

pub trait CloneProcessController: ProcessController {
    fn clone_process_controller(&self) -> Box<dyn CloneProcessController>;
}

impl<F> CloneProcessController for F
where
    F: ProcessController + Clone + 'static,
{
    fn clone_process_controller(&self) -> Box<dyn CloneProcessController> {
        Box::new(self.clone())
    }
}
