//! `ContextBag` — type-erased, thread-safe dependency store.
//!
//! Handlers retrieve shared resources (DB pools, config, caches)
//! from the `ContextBag` during `Serve::create()`.
//!
//! The bag always carries the two engine-minimum elements: the injected
//! [`Clock`] and the shared RNG stream. They are struct fields, not map
//! entries, so a host cannot forget to store them. [`ContextBag::from_ctx`]
//! and [`ContextBag::with_ctx`] take both from a valtron [`Ctx`], so a request
//! handled inside a task reads the engine's time and draws from the engine's
//! seeded stream.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use foundation_core::compati::Mutex;
use foundation_core::valtron::{Clock, Ctx};
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Type-erased store for shared resources, plus the engine-minimum clock and
/// RNG.
pub struct ContextBag {
    store: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
    clock: Arc<dyn Clock>,
    rng: Arc<Mutex<ChaCha8Rng>>,
}

impl ContextBag {
    /// Create an empty `ContextBag` with the given clock.
    ///
    /// The RNG starts fresh with seed `0`. Use [`ContextBag::from_ctx`] to
    /// share a valtron task's seeded stream instead.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            clock,
            rng: Arc::new(Mutex::new(ChaCha8Rng::seed_from_u64(0))),
        }
    }

    /// Create a `ContextBag` and populate it via a closure.
    #[must_use]
    pub fn build(clock: Arc<dyn Clock>, f: impl FnOnce(&mut Self)) -> Self {
        let mut bag = Self::new(clock);
        f(&mut bag);
        bag
    }

    /// A new bag with an empty map, sharing `ctx`'s clock and RNG stream.
    ///
    /// This is the path a valtron task uses: the request reads the engine's
    /// time and draws from the engine's seeded stream.
    #[must_use]
    pub fn from_ctx(ctx: &Ctx) -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            clock: ctx.clock(),
            rng: ctx.rng(),
        }
    }

    /// A new bag over a *copied* map, with `ctx`'s clock and RNG stream.
    ///
    /// The map copy follows the `Ctx::fork` rule: later inserts into either bag
    /// stay private, and the `Arc` values stay shared. This is the bridge a
    /// valtron task uses to keep the items a handler registered while it takes
    /// the task's clock and RNG.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn with_ctx(&self, ctx: &Ctx) -> Self {
        Self {
            store: RwLock::new(self.copied_store()),
            clock: ctx.clock(),
            rng: ctx.rng(),
        }
    }

    /// A new bag over a *copied* map, keeping this bag's clock and RNG.
    ///
    /// The map copy follows the `Ctx::fork` rule: later inserts into either bag
    /// stay private, and the `Arc` values stay shared.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn fork(&self) -> Self {
        Self {
            store: RwLock::new(self.copied_store()),
            clock: Arc::clone(&self.clock),
            rng: Arc::clone(&self.rng),
        }
    }

    /// Copy the store map.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    fn copied_store(&self) -> HashMap<TypeId, Arc<dyn Any + Send + Sync>> {
        self.store.read().unwrap().clone()
    }

    /// The injected engine clock.
    #[must_use]
    pub fn clock(&self) -> Arc<dyn Clock> {
        Arc::clone(&self.clock)
    }

    /// The shared RNG stream. Lock it to draw:
    /// `bag.rng().lock().next_u64()`.
    #[must_use]
    pub fn rng(&self) -> Arc<Mutex<ChaCha8Rng>> {
        Arc::clone(&self.rng)
    }

    /// Panic if an engine-minimum element is missing, and name every missing
    /// element.
    ///
    /// Both elements are non-optional fields, so the constructor signature
    /// proves them. This guard keeps the same boundary shape as
    /// `foundation_core::valtron::Ctx::assert_minimum`.
    ///
    /// # Panics
    ///
    /// Panics when the clock or the RNG is missing.
    pub fn assert_minimum(&self) {
        let _ = (self.clock(), self.rng());
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

// Safety: RwLock + HashMap<TypeId, Arc<dyn Any + Send + Sync>>, Arc<dyn Clock>
// and Arc<Mutex<ChaCha8Rng>> are Send + Sync.
unsafe impl Send for ContextBag {}
unsafe impl Sync for ContextBag {}
