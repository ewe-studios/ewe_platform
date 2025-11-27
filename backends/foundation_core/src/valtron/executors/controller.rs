use std::time;

pub trait ProcessController {
    /// yield_process is expected to communicate with the
    /// relevant thread to yield the thread for other
    /// processes to take over, as the relevant underlying
    /// operation is not ready to continue or is idle.
    ///
    /// This will allow us efficiently yield the thread without
    /// busy spinlooping hugging CPU time that could efficiently
    /// be allocated.
    ///
    /// The implementation on the platform will decide how it will
    /// achieve this but can use the relevant platform specific support
    /// to achive this. Think of this to be similar to thread::yield_now().
    fn yield_process(&self);

    /// [`yield_for`] specifically exits to block the relevant
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
