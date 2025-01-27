use std::{thread, time};

pub trait ProcessController {
    fn yield_process(&self);
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

#[derive(Default, Clone)]
pub struct ThreadYield;

impl ProcessController for ThreadYield {
    fn yield_process(&self) {
        thread::yield_now();
    }

    fn yield_for(&self, dur: time::Duration) {
        thread::sleep(dur);
    }
}
