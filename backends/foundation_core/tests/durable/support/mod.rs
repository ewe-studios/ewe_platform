//! A small Given/When/Then helper for driver scenarios.
//!
//! Feature tests use this helper so one scenario can run on every driver. A later feature adds
//! the driver matrix. This module proves the shape now.

/// A scenario bound to one driver.
pub struct Scenario<D> {
    driver: D,
}

impl<D> Scenario<D> {
    /// Start a scenario with a driver.
    #[must_use]
    pub fn given(driver: D) -> Self {
        Self { driver }
    }

    /// Run the `when` step with the driver.
    pub fn when<T>(self, step: impl FnOnce(&D) -> T) -> T {
        step(&self.driver)
    }
}

/// Assert a `then` condition.
///
/// # Panics
///
/// Panics when the condition is false.
pub fn then(condition: bool, message: &str) {
    assert!(condition, "{message}");
}
