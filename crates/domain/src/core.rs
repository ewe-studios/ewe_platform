use std::sync;

use crate::domains;

/// `CoreExecutor` provides a core structure for handling and managing
/// execution of all types implementing [`domains::TaskExecutor`].
///
/// This allows us implement [`domains::TaskExecutor`] on all core
/// parts like [`crate::servicer::DServicer`], [`crate::domains::UseCaseExecutor`]
/// which then allows these different systems to make progress everything
/// the [`CoreExecutor::run_all`] is called.
///
/// This becomes useful in non-async supporting environments like WASM and even the
/// web where blocking the main thread can be disasterous.
pub struct CoreExecutor {
    executors: sync::Mutex<Vec<Box<dyn domains::TaskExecutor>>>,
}

impl Default for CoreExecutor {
    fn default() -> Self {
        Self {
            executors: sync::Mutex::new(Vec::new()),
        }
    }
}

impl CoreExecutor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs all registered task executors.
    ///
    /// # Panics
    ///
    /// Panics if the executor lock is poisoned.
    pub fn run_all(&mut self) {
        let mut executors = self.executors.lock().unwrap();
        for executor in executors.iter_mut() {
            executor.run_tasks();
        }
    }

    /// Registers a new task executor.
    ///
    /// # Panics
    ///
    /// Panics if the executor lock is poisoned.
    pub fn register(&mut self, executor: Box<dyn domains::TaskExecutor>) {
        let mut executors = self.executors.lock().unwrap();
        executors.push(executor);
    }
}
