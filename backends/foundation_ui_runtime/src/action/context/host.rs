//! Access the host services for one render or action call.

use std::{any::Any, sync::Arc};

use foundation_compact::rng::ChaCha8Rng;
use foundation_context::{sync::Mutex, Clock, Config, Ctx};

use super::Context;

impl Context {
    /// Read the host service of this type. See [`Ctx::get`].
    #[must_use]
    pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.inner().get::<T>()
    }

    /// Store the service under its type. See [`Ctx::store`].
    pub fn store<T: Any + Send + Sync>(&self, value: T) {
        self.inner().store(value);
    }

    /// Store the given handle and return the previous handle. See [`Ctx::insert`].
    pub fn insert<T: Any + Send + Sync>(&self, value: Arc<T>) -> Option<Arc<T>> {
        self.inner().insert(value)
    }

    /// Replace the service and return the previous handle. See [`Ctx::get_and_store`].
    #[must_use]
    pub fn get_and_store<T: Any + Send + Sync>(&self, value: T) -> Option<Arc<T>> {
        self.inner().get_and_store(value)
    }

    /// Clone the stored value. See [`Ctx::get_cloned`].
    #[must_use]
    pub fn get_cloned<T: Any + Clone + Send + Sync>(&self) -> Option<T> {
        self.inner().get_cloned::<T>()
    }

    /// Return the current handle or install the supplied value. See [`Ctx::get_or_insert`].
    #[must_use]
    pub fn get_or_insert<T: Any + Send + Sync>(&self, value: T) -> Arc<T> {
        self.inner().get_or_insert(value)
    }

    /// Remove the service and return its handle. See [`Ctx::remove`].
    #[must_use]
    pub fn remove<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.inner().remove::<T>()
    }

    /// Check whether the host contains this type. See [`Ctx::contains`].
    #[must_use]
    pub fn contains<T: Any + Send + Sync>(&self) -> bool {
        self.inner().contains::<T>()
    }

    /// Store the supplied value under its type. See [`Ctx::store`].
    pub fn set<T: Any + Send + Sync>(&self, value: T) {
        self.inner().store(value);
    }

    /// Return the host clock. See [`Ctx::clock`].
    #[must_use]
    pub fn clock(&self) -> Arc<dyn Clock> {
        self.inner().clock()
    }

    /// Return the host clock if present. See [`Ctx::try_clock`].
    #[must_use]
    pub fn try_clock(&self) -> Option<Arc<dyn Clock>> {
        self.inner().try_clock()
    }

    /// Return the shared host configuration. See [`Ctx::config`].
    #[must_use]
    pub fn config(&self) -> Arc<Config> {
        self.inner().config()
    }

    /// Return the host random number generator. See [`Ctx::rng`].
    #[must_use]
    pub fn rng(&self) -> Arc<Mutex<ChaCha8Rng>> {
        self.inner().rng()
    }

    /// Return the host random number generator if present. See [`Ctx::try_rng`].
    #[must_use]
    pub fn try_rng(&self) -> Option<Arc<Mutex<ChaCha8Rng>>> {
        self.inner().try_rng()
    }

    /// Check that the host has its clock, configuration and generator. See [`Ctx::assert_minimum`].
    pub fn assert_minimum(&self) {
        self.inner().assert_minimum();
    }

    /// Copy the service map and share the services it contains. See [`Ctx::fork`].
    #[must_use]
    pub fn fork(&self) -> Ctx {
        self.inner().fork()
    }

    /// Create an empty host map with the same clock, configuration and generator. See [`Ctx::with_empty_map`].
    #[must_use]
    pub fn with_empty_map(&self) -> Ctx {
        self.inner().with_empty_map()
    }

    /// Copy the host entries and use the standard services from the supplied context. See [`Ctx::rebase`].
    #[must_use]
    pub fn rebase(&self, sources: &Ctx) -> Ctx {
        self.inner().rebase(sources)
    }

    /// Copy the host entries and use the supplied clock. See [`Ctx::with_clock`].
    #[must_use]
    pub fn with_clock(&self, clock: Arc<dyn Clock>) -> Ctx {
        self.inner().with_clock(clock)
    }
}
