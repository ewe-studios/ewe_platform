//! Typed values and handles for the shared runtime.

use super::{Id, MultiThreadedRuntime};
use std::sync::{Arc, PoisonError, RwLock};

pub(super) struct Storage<T> {
    pub runtime: Arc<MultiThreadedRuntime>,
    pub id: Id,
    pub computed: bool,
    pub value: RwLock<Option<T>>,
}

/// Reads one signal or a cached computation.
pub struct SignalGetter<T>(pub(super) Arc<Storage<T>>);

impl<T> Clone for SignalGetter<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: Clone> SignalGetter<T> {
    /// Returns the current value. An active computation or effect subscribes to changes.
    ///
    /// # Panics
    /// Can propagate a panic from the value's clone or a user computation.
    pub fn get(&self) -> T {
        if self.0.computed {
            self.0.runtime.refresh_computed(self.0.id);
        }
        let value = self.0.value.read().unwrap_or_else(PoisonError::into_inner);
        self.0.runtime.track(self.0.id);
        value
            .as_ref()
            .expect("Published getter has an initialized value")
            .clone()
    }

    /// Reads the current value without subscribing.
    ///
    /// # Panics
    /// Can propagate a panic from the value's clone or a user computation.
    pub fn get_untracked(&self) -> T {
        self.0
            .value
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .expect("Published getter has an initialized value")
            .clone()
    }
}

/// Writes one signal and publishes changes to its runtime.
pub struct SignalSetter<T>(pub(super) Arc<Storage<T>>);

impl<T> Clone for SignalSetter<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: Clone + PartialEq> SignalSetter<T> {
    /// Replaces the current value. Equal values keep their existing notifications.
    pub fn set(&self, value: T) {
        let mut stored = self.0.value.write().unwrap_or_else(PoisonError::into_inner);
        let changed = stored.as_ref() != Some(&value);
        let old = stored.replace(value);
        if changed {
            self.0.runtime.changed(self.0.id);
        }
        drop(stored);
        drop(old);
    }

    /// Applies one change and discards its unit result.
    pub fn update(&self, change: impl FnOnce(&mut T)) {
        self.update_with(change);
    }

    /// Applies one change under the value's write lock and returns the closure's result.
    /// The closure must only use its supplied value. A panic discards the modified copy.
    ///
    /// # Panics
    /// Propagates a panic from the caller or the value's clone or comparison.
    pub fn update_with<R>(&self, change: impl FnOnce(&mut T) -> R) -> R {
        let mut stored = self.0.value.write().unwrap_or_else(PoisonError::into_inner);
        let mut candidate = stored
            .as_ref()
            .expect("A writable signal always has a value")
            .clone();
        let result = change(&mut candidate);
        let changed = stored.as_ref() != Some(&candidate);
        let old = stored.replace(candidate);
        if changed {
            self.0.runtime.changed(self.0.id);
        }
        drop(stored);
        drop(old);
        result
    }
}
