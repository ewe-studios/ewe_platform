//! The shared context: a typed map plus a clock, a config and a random number generator.
//!
//! `foundation_core::valtron` re-exports these items.
//!
//! [`Ctx`] is a cloneable `Send + Sync` handle over a [`CtxInner`]: an
//! interior-mutable type map keyed by [`TypeId`], plus three engine-seeded
//! standard elements — a [`Clock`] (default [`MonotonicClock`]), a [`Config`],
//! and a shared [`ChaCha8Rng`].
//! Two operations distinguish the sharing modes:
//!
//! - [`Ctx::clone`] **shares** the map (a cheap `Arc` refcount bump) — used
//!   within one `ExecutionIterator`'s subtree, so a task and its spawned sub-tasks
//!   observe each other's writes.
//! - [`Ctx::fork`] **copies** the map into a fresh handle (a new `HashMap` whose
//!   `Arc` values are still shared) — used by the engine at each
//!   `ExecutionIterator` boundary, so sibling iterators never mutate each other's
//!   map while the standard elements stay shared.
//!
//! `Ctx` is passed **by value** (an `Arc` clone) through the task traits — never
//! `&Ctx`/`&mut Ctx` — so an async transition's future simply owns its clone.
//!
//! ## Sub-task sharing example
//!
//! ```
//! use foundation_context::Ctx;
//! use std::sync::Arc;
//!
//! let engine_ctx = Ctx::host();
//!
//! // Within one ExecutionIterator, the wrapper hands its ctx to the inner task
//! // by clone() — both handles point at the same map.
//! engine_ctx.insert(Arc::new(42u64));
//! let sub_task_ctx = engine_ctx.clone();
//! assert_eq!(*sub_task_ctx.get::<u64>().unwrap(), 42);
//!
//! // A sibling ExecutionIterator gets a *forked* (structurally copied) map, so
//! // entries the parent adds afterwards stay invisible to it; the standard
//! // elements (clock/config/rng) remain shared Arc's.
//! let sibling = engine_ctx.fork();
//! engine_ctx.insert(Arc::new(7u32));
//! assert!(!sibling.contains::<u32>());
//! assert!(Arc::ptr_eq(&engine_ctx.clock(), &sibling.clock()));
//! ```
//!
//! # Requirements
//!
//! This crate needs `std` (`HashMap`, `Arc`, `TypeId`, `dyn Any`). It always enables the
//! `std` feature of `foundation_compact`.

/// Lock types for the target. Native uses `std::sync`. Wasm uses `foundation_nostd`.
pub mod sync {
    #[cfg(target_family = "wasm")]
    pub use foundation_nostd::primitives::{Mutex, RwLock};

    #[cfg(not(target_family = "wasm"))]
    pub use std::sync::{Mutex, RwLock};
}

use core::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use crate::sync::{Mutex, RwLock};
use foundation_compact::rng::{ChaCha8Rng, SeedableRng};
use foundation_compact::{Duration, Instant, SystemTime};

/// A read-only clock, seeded by the engine and shared as an `Arc<dyn Clock>`.
///
/// Tasks read the current monotonic instant, the time elapsed since the clock
/// was seeded, or the current wall-clock time. The trait is object-safe so the
/// engine can seed any implementation — the real [`MonotonicClock`] or a test
/// mock — and swap it at init without changing [`Ctx`]'s shape.
pub trait Clock: Send + Sync + 'static {
    /// The current instant.
    #[must_use]
    fn now(&self) -> Instant;

    /// The time elapsed since this clock was seeded.
    #[must_use]
    fn elapsed(&self) -> Duration;

    /// The current wall-clock time.
    ///
    /// This is a required method. A default that read the real clock would be a
    /// silent fallback to the global, so every implementation must choose a
    /// value: the real time in a host backend, a fixed origin in a test backend.
    #[must_use]
    fn wall_now(&self) -> SystemTime;
}

/// The default [`Clock`]: a real, [`foundation_compact`]-backed clock.
///
/// On native, `Instant` is `std::time::Instant`; on wasm it is the JS-backed
/// performance clock, so [`Clock::now`] and [`Clock::elapsed`] work across web
/// and non-web endpoints without any platform branch here. [`Clock::wall_now`]
/// reads the real wall clock; this type is the one place in the engine where
/// that read stays. `Copy`, no interior mutation — safe to share across threads
/// and tasks.
#[derive(Debug, Clone, Copy)]
pub struct MonotonicClock {
    origin: Instant,
}

impl MonotonicClock {
    /// A clock whose origin is "now".
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Clock for MonotonicClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn elapsed(&self) -> Duration {
        self.origin.elapsed()
    }

    fn wall_now(&self) -> SystemTime {
        SystemTime::now()
    }
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Read-only, engine-seeded configuration shared across tasks.
///
/// First-cut placeholder: the engine owns the real configuration source and will
/// populate this type in a later feature. It exists now so [`Ctx::config`] has a
/// stable, cloneable handle.
#[derive(Debug, Clone, Copy, Default)]
pub struct Config;

impl Config {
    /// A default (empty) configuration.
    #[must_use]
    pub fn new() -> Self {
        Config
    }
}

/// The engine-provided context. See the [module docs](self) for the sharing
/// model (`clone` shares, `fork` copies).
pub struct Ctx(Arc<CtxInner>);

/// One of the engine-minimum elements that every task context must carry.
///
/// [`Ctx::assert_minimum`] names the missing element in its panic message. The
/// compiler checks the [`Ctx::fixed`] clock argument, but a context can reach a
/// task boundary from a path the compiler cannot check. The boundary checks the
/// full minimum set before the task starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MinimumElement {
    /// The injected [`Clock`].
    Clock,
    /// The engine [`Config`].
    Config,
    /// The shared RNG stream.
    Rng,
}

impl MinimumElement {
    /// The element's name, as [`Ctx::assert_minimum`] reports it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Clock => "clock",
            Self::Config => "config",
            Self::Rng => "rng",
        }
    }
}

/// Interior of a [`Ctx`]. `Send + Sync` because every field is `Send + Sync`.
///
/// The standard elements are `Option` fields, not map entries. A public
/// constructor always fills them; `None` exists only so
/// [`Ctx::incomplete_for_test`] can prove that [`Ctx::assert_minimum`] rejects a
/// context from an unchecked path.
pub(crate) struct CtxInner {
    /// General extension point: arbitrary `Arc<T>` entries keyed by `TypeId`.
    map: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
    /// Engine-seeded standard elements.
    clock: Option<Arc<dyn Clock>>,
    config: Option<Arc<Config>>,
    rng: Option<Arc<Mutex<ChaCha8Rng>>>,
}

impl Ctx {
    /// An empty context: no map entries, the given clock, a default config, and
    /// a deterministic RNG with the fixed seed `0`.
    ///
    /// The name says what it does: the seed is fixed, so every context from this
    /// constructor draws the same stream. Use it in tests, where a run must
    /// repeat. A host must not use it; a host calls [`Ctx::host`]. A
    /// deterministic simulation test (DST) that chooses its seed calls
    /// [`Ctx::with_seed`].
    ///
    /// The clock is a required argument, so the compiler rejects a context with
    /// no clock (AGENTS rule 11).
    #[must_use]
    pub fn fixed(clock: Arc<dyn Clock>) -> Self {
        Self::with_seed(clock, 0)
    }

    /// An empty context with the given clock and an RNG seeded from `seed`.
    ///
    /// Two contexts with the same seed draw the same values. A DST uses this to
    /// choose the seed, so it can replay a run.
    #[must_use]
    pub fn with_seed(clock: Arc<dyn Clock>, seed: u64) -> Self {
        Self::with_rng(clock, ChaCha8Rng::seed_from_u64(seed))
    }

    /// A context with the real [`MonotonicClock`] and an RNG seeded from the
    /// entropy backend of `foundation_compact`.
    ///
    /// This is the one explicit host constructor. Call it only at a host entry
    /// point, where real time and real entropy are the correct sources. Library
    /// code must use the [`Ctx`] it is given. Two calls draw different streams.
    ///
    /// # Panics
    ///
    /// Panics if the entropy source fails. Use [`Ctx::try_host`] to handle the
    /// failure.
    #[must_use]
    pub fn host() -> Self {
        Self::try_host().expect("the entropy source failed while Ctx::host seeded the RNG")
    }

    /// Like [`Ctx::host`], but returns the error when the entropy source fails.
    ///
    /// # Errors
    ///
    /// Returns the [`foundation_compact::entropy::Error`] of the entropy backend.
    pub fn try_host() -> Result<Self, foundation_compact::entropy::Error> {
        let mut seed = [0u8; 32];
        foundation_compact::entropy::fill(&mut seed)?;
        Ok(Self::with_rng(
            Arc::new(MonotonicClock::new()),
            ChaCha8Rng::from_seed(seed),
        ))
    }

    /// An empty context with the given clock and RNG.
    fn with_rng(clock: Arc<dyn Clock>, rng: ChaCha8Rng) -> Self {
        Ctx(Arc::new(CtxInner {
            map: RwLock::new(HashMap::new()),
            clock: Some(clock),
            config: Some(Arc::new(Config::new())),
            rng: Some(Arc::new(Mutex::new(rng))),
        }))
    }

    /// A context made by [`Ctx::fixed`] (fixed seed `0`) and then filled by `f`.
    ///
    /// `f` receives the new context. Use it to store the first values in one
    /// expression.
    #[must_use]
    pub fn build(clock: Arc<dyn Clock>, f: impl FnOnce(&Ctx)) -> Self {
        let ctx = Self::fixed(clock);
        f(&ctx);
        ctx
    }

    /// Panic if an engine-minimum element is missing, and name every missing
    /// element.
    ///
    /// Call this at a task execute or spawn boundary, where a [`Ctx`] arrives
    /// from a path the compiler cannot check.
    ///
    /// # Panics
    ///
    /// Panics when the clock, the config, or the RNG is missing.
    pub fn assert_minimum(&self) {
        let mut missing: Vec<&'static str> = Vec::new();
        if self.0.clock.is_none() {
            missing.push(MinimumElement::Clock.name());
        }
        if self.0.config.is_none() {
            missing.push(MinimumElement::Config.name());
        }
        if self.0.rng.is_none() {
            missing.push(MinimumElement::Rng.name());
        }
        assert!(
            missing.is_empty(),
            "Ctx is missing engine-minimum element(s): {missing:?}"
        );
    }

    /// A context that deliberately omits one engine-minimum element.
    ///
    /// WHY: [`Ctx::assert_minimum`] must reject a context from an unchecked
    /// path. Tests need such a context to prove the check.
    /// WHAT: an empty context, the given clock, and `None` for the named
    /// element.
    ///
    /// Production code must not call this function.
    #[doc(hidden)]
    #[must_use]
    pub fn incomplete_for_test(clock: Arc<dyn Clock>, missing: MinimumElement) -> Self {
        Ctx(Arc::new(CtxInner {
            map: RwLock::new(HashMap::new()),
            clock: (missing != MinimumElement::Clock).then_some(clock),
            config: (missing != MinimumElement::Config).then(|| Arc::new(Config::new())),
            rng: (missing != MinimumElement::Rng)
                .then(|| Arc::new(Mutex::new(ChaCha8Rng::seed_from_u64(0)))),
        }))
    }

    /// Insert `value` under its type, replacing any prior entry and returning it.
    ///
    /// Entries are keyed by `TypeId`, so two distinct `T`s never collide. The
    /// prior entry (if any) is returned for callers that care; like
    /// `HashMap::insert`, ignoring it is fine.
    ///
    /// # Panics
    /// Panics if the internal map lock is poisoned.
    pub fn insert<T: Any + Send + Sync>(&self, value: Arc<T>) -> Option<Arc<T>> {
        self.0
            .map
            .write()
            .expect("Ctx map poisoned")
            .insert(TypeId::of::<T>(), value)
            .and_then(|prior| prior.downcast::<T>().ok())
    }

    /// Store `value` under its type, replacing any prior entry.
    ///
    /// This is `insert(Arc::new(value))`. The prior entry is dropped; use
    /// [`Ctx::get_and_store`] to get it back.
    ///
    /// # Panics
    /// Panics if the internal map lock is poisoned.
    pub fn store<T: Any + Send + Sync>(&self, value: T) {
        drop(self.insert(Arc::new(value)));
    }

    /// Store `value` under its type and return the prior entry, if any.
    ///
    /// This is [`Ctx::store`] for a caller that needs the replaced value. It is
    /// `insert(Arc::new(value))`.
    ///
    /// # Panics
    /// Panics if the internal map lock is poisoned.
    #[must_use]
    pub fn get_and_store<T: Any + Send + Sync>(&self, value: T) -> Option<Arc<T>> {
        self.insert(Arc::new(value))
    }

    /// The `Arc<T>` stored under `T`, or `None` if absent or of a different type.
    ///
    /// # Panics
    /// Panics if the internal map lock is poisoned.
    #[must_use]
    pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.0
            .map
            .read()
            .expect("Ctx map poisoned")
            .get(&TypeId::of::<T>())
            .cloned()
            .and_then(|value| value.downcast::<T>().ok())
    }

    /// A clone of the value stored under `T`, or `None` if absent.
    ///
    /// # Panics
    /// Panics if the internal map lock is poisoned.
    #[must_use]
    pub fn get_cloned<T: Any + Clone + Send + Sync>(&self) -> Option<T> {
        self.get::<T>().map(|value| T::clone(&value))
    }

    /// Insert `value` under `T` if absent; return the live `Arc<T>` either way.
    ///
    /// # Panics
    /// Panics if the internal map lock is poisoned.
    #[must_use]
    pub fn get_or_insert<T: Any + Send + Sync>(&self, value: T) -> Arc<T> {
        let mut map = self.0.map.write().expect("Ctx map poisoned");
        if let Some(existing) = map.get(&TypeId::of::<T>()) {
            if let Ok(existing) = existing.clone().downcast::<T>() {
                return existing;
            }
        }
        let arc = Arc::new(value);
        map.insert(TypeId::of::<T>(), arc.clone());
        arc
    }

    /// Remove and return the `Arc<T>` stored under `T`, if present.
    ///
    /// # Panics
    /// Panics if the internal map lock is poisoned.
    #[must_use]
    pub fn remove<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.0
            .map
            .write()
            .expect("Ctx map poisoned")
            .remove(&TypeId::of::<T>())
            .and_then(|value| value.downcast::<T>().ok())
    }

    /// Whether a `T` is present in the map.
    ///
    /// # Panics
    /// Panics if the internal map lock is poisoned.
    #[must_use]
    pub fn contains<T: Any + Send + Sync>(&self) -> bool {
        self.0
            .map
            .read()
            .expect("Ctx map poisoned")
            .contains_key(&TypeId::of::<T>())
    }

    /// The shared, read-only clock.
    ///
    /// # Panics
    ///
    /// Panics if the clock is missing. Every public constructor installs it;
    /// call [`Ctx::assert_minimum`] at a boundary that accepts an unchecked
    /// context.
    #[must_use]
    pub fn clock(&self) -> Arc<dyn Clock> {
        self.0.clock.clone().expect(
            "Ctx is missing its engine-minimum clock; call Ctx::assert_minimum at the boundary",
        )
    }

    /// The shared, read-only configuration.
    ///
    /// # Panics
    ///
    /// Panics if the config is missing. Every public constructor installs it;
    /// call [`Ctx::assert_minimum`] at a boundary that accepts an unchecked
    /// context.
    #[must_use]
    pub fn config(&self) -> Arc<Config> {
        self.0.config.clone().expect(
            "Ctx is missing its engine-minimum config; call Ctx::assert_minimum at the boundary",
        )
    }

    /// The single shared RNG stream. Lock it to draw: `ctx.rng().lock().next_u64()`.
    ///
    /// No method returns the guard, so a lock can never be held across an
    /// `.await`.
    ///
    /// # Panics
    ///
    /// Panics if the RNG is missing. Every public constructor installs it;
    /// call [`Ctx::assert_minimum`] at a boundary that accepts an unchecked
    /// context.
    #[must_use]
    pub fn rng(&self) -> Arc<Mutex<ChaCha8Rng>> {
        self.0.rng.clone().expect(
            "Ctx is missing its engine-minimum rng; call Ctx::assert_minimum at the boundary",
        )
    }

    /// A new handle over a *copied* map.
    ///
    /// The fresh `HashMap` structurally isolates this handle (insert/remove are
    /// private to it), while the `Arc` values and the standard elements stay
    /// shared — so entries inserted *through* this fork remain reachable from the
    /// parent's `Arc` if the parent already held the same `Arc`.
    ///
    /// # Panics
    /// Panics if the internal map lock is poisoned.
    #[must_use]
    pub fn fork(&self) -> Ctx {
        let map = self.0.map.read().expect("Ctx map poisoned").clone();
        Ctx(Arc::new(CtxInner {
            map: RwLock::new(map),
            clock: self.0.clock.clone(),
            config: self.0.config.clone(),
            rng: self.0.rng.clone(),
        }))
    }

    /// A new handle over an *empty* map. It shares this handle's clock, config
    /// and RNG.
    ///
    /// Use it when a request needs a fresh map but must read the same time and
    /// draw from the same stream.
    #[must_use]
    pub fn with_empty_map(&self) -> Ctx {
        Ctx(Arc::new(CtxInner {
            map: RwLock::new(HashMap::new()),
            clock: self.0.clock.clone(),
            config: self.0.config.clone(),
            rng: self.0.rng.clone(),
        }))
    }

    /// A new handle over a *copied* map of this handle, with the clock, config
    /// and RNG of `sources`.
    ///
    /// The map copy follows the [`Ctx::fork`] rule: later inserts into either
    /// handle stay private, and the `Arc` values stay shared. A task uses this to
    /// keep the values a handler stored while it takes the engine's clock and
    /// RNG.
    ///
    /// # Panics
    /// Panics if the internal map lock is poisoned.
    #[must_use]
    pub fn rebase(&self, sources: &Ctx) -> Ctx {
        let map = self.0.map.read().expect("Ctx map poisoned").clone();
        Ctx(Arc::new(CtxInner {
            map: RwLock::new(map),
            clock: sources.0.clock.clone(),
            config: sources.0.config.clone(),
            rng: sources.0.rng.clone(),
        }))
    }

    /// A new handle over a *copied* map, the same config and RNG, and `clock`.
    ///
    /// The map copy follows the [`Ctx::fork`] rule: this handle and `self` do not
    /// see each other's later inserts. The `config` and `rng` `Arc`s stay
    /// shared. `self` keeps its own clock, so a caller can build several handles
    /// from one root.
    ///
    /// This is the startup hook for a deterministic simulation test (DST): the
    /// host passes the returned handle to `initialize_pool_with_ctx`, and every
    /// task fork reads the same clock from [`Ctx::clock`].
    ///
    /// # Panics
    /// Panics if the internal map lock is poisoned.
    #[must_use]
    pub fn with_clock(&self, clock: Arc<dyn Clock>) -> Ctx {
        let map = self.0.map.read().expect("Ctx map poisoned").clone();
        Ctx(Arc::new(CtxInner {
            map: RwLock::new(map),
            clock: Some(clock),
            config: self.0.config.clone(),
            rng: self.0.rng.clone(),
        }))
    }
}

impl Clone for Ctx {
    /// Shares the map: both handles point at the same `Arc<CtxInner>`.
    fn clone(&self) -> Self {
        Ctx(self.0.clone())
    }
}
