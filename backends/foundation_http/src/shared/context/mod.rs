//! `ContextBag` — type-erased, thread-safe dependency store.
//!
//! Handlers retrieve shared resources (DB pools, config, caches)
//! from the `ContextBag` during `Serve::create()`.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Type-erased store for shared resources.
pub struct ContextBag {
    store: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl ContextBag {
    /// Create an empty `ContextBag`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
        }
    }

    /// Create a `ContextBag` and populate it via a closure.
    #[must_use]
    pub fn build(f: impl FnOnce(&mut Self)) -> Self {
        let mut bag = Self::new();
        f(&mut bag);
        bag
    }

    /// Store a value in the bag, replacing any existing value of the same type.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn store<T: Any + Send + Sync>(&self, value: T) {
        let mut store = self.store.write().unwrap();
        store.insert(TypeId::of::<T>(), Arc::new(value));
    }

    /// Get a reference to a stored value, wrapped in `Arc`.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        let store = self.store.read().unwrap();
        store
            .get(&TypeId::of::<T>())
            .and_then(|v| Arc::clone(v).downcast::<T>().ok())
    }

    /// Get a cloned copy of a stored value (requires `T: Clone`).
    #[must_use]
    pub fn get_cloned<T: Any + Clone + Send + Sync>(&self) -> Option<T> {
        self.get::<T>().map(|v| T::clone(&v))
    }

    /// Remove and return a stored value.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn remove<T: Any + Send + Sync>(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        let mut store = self.store.write().unwrap();
        store.remove(&TypeId::of::<T>())
    }

    /// Check if a type is stored in the bag.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn contains<T: Any + Send + Sync>(&self) -> bool {
        let store = self.store.read().unwrap();
        store.contains_key(&TypeId::of::<T>())
    }
}

impl Default for ContextBag {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: RwLock + HashMap<TypeId, Arc<dyn Any + Send + Sync>> is Send + Sync
unsafe impl Send for ContextBag {}
unsafe impl Sync for ContextBag {}
