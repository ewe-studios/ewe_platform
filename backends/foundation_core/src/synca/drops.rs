/// AbortIfPanic aborts the current process on the thread
/// if it ever gets dropped.
/// The only way to avoid it is to call std::mem::forget on the
pub struct AbortIfPanic;

impl Default for AbortIfPanic {
    fn default() -> Self {
        Self
    }
}

impl Drop for AbortIfPanic {
    fn drop(&mut self) {
        tracing::debug!("detected unexpected panic; aborting");
        ::std::process::abort();
    }
}

/// RunOnDrop implements a type that runs a function when
/// it gets dropped, providing a similar convention to go's defer.
pub struct RunOnDrop<F: FnOnce() -> ()>(Option<F>);

impl<F: FnOnce() -> ()> RunOnDrop<F> {
    pub fn new(f: F) -> Self {
        Self(Some(f))
    }
}

impl<F: FnOnce() -> ()> Drop for RunOnDrop<F> {
    fn drop(&mut self) {
        if let Some(cb) = self.0.take() {
            cb();
        }
    }
}
