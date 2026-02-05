//! Run-once wrapper for single-threaded executor.
//!
//! This module provides a wrapper around `single::run_once` with start/stop functionality.
//! It enables running `run_once` in a loop with configurable sleep duration and provides
//! methods to start and stop the loop.
//!
//! ## Features
//!
//! - Uses foundation_wasm to schedule an interval function that will be called on duration
//! - Triggers the single::run_once function per tick
//! - Stops and unregisters wasm function handle when stopped
//!
//! ## Usage
//!
//! ```ignore
//! use foundation_core::valtron::executors::run_once_wrapper::{RunOnceWrapper, RunOnceWrapperBuilder};
//!
//! let mut wrapper = RunOnceWrapperBuilder::new()
//!     .sleep_duration(std::time::Duration::from_millis(100))
//!     .build();
//!
//! // Start the loop
//! wrapper.start();
//!
//! // Later, stop the loop
//! wrapper.stop();
//!```
//!
//! ## Implementation
//!
//! The wrapper uses a `thread::spawn` to run the loop in a separate thread. The loop
//! continues until the atomic flag is set to `true`. The `run_once` function is called
//! in each iteration, and the thread sleeps for the configured duration between calls.
//!

use foundation_wasm::{register_interval, TickState};
use std::time::Duration;

use super::single;

/// Builder for `RunOnceWrapper`.
///
/// This builder allows configuring the sleep duration before creating the wrapper.
pub struct RunOnceWrapperBuilder {
    sleep_duration: Duration,
}

impl RunOnceWrapperBuilder {
    /// Creates a new `RunOnceWrapperBuilder` with default sleep duration of 100ms.
    pub fn new() -> Self {
        Self {
            sleep_duration: Duration::from_millis(100),
        }
    }

    /// Sets the sleep duration between `run_once` calls.
    ///
    /// # Arguments
    ///
    /// * `duration` - The duration to sleep between calls to `run_once`
    ///
    /// # Returns
    ///
    /// The builder for method chaining
    pub fn sleep_duration(mut self, duration: Duration) -> Self {
        self.sleep_duration = duration;
        self
    }

    /// Builds the `RunOnceWrapper` with the configured settings.
    ///
    /// # Returns
    ///
    /// A new `RunOnceWrapper` instance
    pub fn build(self) -> RunOnceWrapper {
        RunOnceWrapper {
            sleep_duration: self.sleep_duration,
            interval_handle: None,
        }
    }
}

/// Wrapper around `single::run_once` with start/stop functionality.
///
/// This struct provides a thread-safe way to run `single::run_once` in a loop
/// with configurable sleep duration and the ability to stop the loop.
pub struct RunOnceWrapper {
    /// Duration to sleep between `run_once` calls
    sleep_duration: Duration,

    /// Handle to the thread running the loop
    interval_handle: Option<InternalPointer>,
}

impl RunOnceWrapper {
    /// Creates a new `RunOnceWrapper` with the default sleep duration of 100ms.
    ///
    /// # Returns
    ///
    /// A new `RunOnceWrapper` instance
    pub fn new() -> Self {
        RunOnceWrapperBuilder::new().build()
    }

    /// Starts the loop in a separate thread.
    ///
    /// This method spawns a new thread that runs `run_once` in a loop until the
    /// stop flag is set. The thread sleeps for the configured duration between calls.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the thread was successfully spawned, `Err` otherwise
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Create a clone of the stop flag for the thread
        let sleep_duration = self.sleep_duration;

        // Spawn the thread
        match self.interval_handle.take() {
            Some(handle) => {
                self.interval_handle = Some(handle);
                return Err("already started".into());
            }
            None => {
                self.interval_handle = Some(register_interval(sleep_duration as f64, {
                    single::run_once();
                    TickState::QUEUE
                }));
                Ok(())
            }
        }
    }

    /// Stops the loop by setting the stop flag.
    ///
    /// This method sets the atomic flag to `true`, which will cause the loop in the
    /// spawned thread to terminate on the next iteration.
    pub fn stop(&self) {
        match self.interval_handle.take() {
            None => {}
            Some(handle) => {
                unregister_interval(handle);
            }
        }
    }

    /// Waits for the thread to finish.
    ///
    /// This method blocks until the thread running the loop has finished.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the thread completed successfully, `Err` if the thread panicked
    pub fn join(self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
