//! Engine-provided, per-poll context handed to every task (feature 04-ctx).
//!
//! [`Ctx`] is a cloneable `Send + Sync` handle over a [`CtxInner`]: an
//! interior-mutable type map keyed by [`TypeId`], plus three engine-seeded
//! standard elements — a [`Clock`] (default [`MonotonicClock`]), a [`Config`],
//! and a shared [`ChaCha8Rng`].
//! Two operations distinguish the sharing modes:
//!
//! - [`Ctx::clone`] **shares** the map (a cheap `Arc` refcount bump) — used
//!   within one ExecutionIterator's subtree, so a task and its spawned sub-tasks
//!   observe each other's writes.
//! - [`Ctx::fork`] **copies** the map into a fresh handle (a new `HashMap` whose
//!   `Arc` values are still shared) — used by the engine at each
//!   ExecutionIterator boundary, so sibling iterators never mutate each other's
//!   map while the standard elements stay shared.
//!
//! `Ctx` is passed **by value** (an `Arc` clone) through the task traits — never
//! `&Ctx`/`&mut Ctx` — so an async transition's future simply owns its clone.
//!
//! ## Sub-task sharing example
//!
//! ```
//! use foundation_core::valtron::Ctx;
//! use std::sync::Arc;
//!
//! let engine_ctx = Ctx::new();
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
//! # Gating
//!
//! The type map and standard elements require `std` (`HashMap`, `Arc`, `TypeId`,
//! `dyn Any`, and a monotonic clock). This mirrors `FutureTask`/`StreamTask`
//! gating; the `alloc`-only relaxation is deferred.

use core::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use crate::compati::Mutex;
use foundation_compact::rng::{ChaCha8Rng, SeedableRng};
use foundation_compact::{Duration, Instant};

/// A read-only monotonic clock, seeded by the engine and shared as an
/// `Arc<dyn Clock>`.
///
/// Tasks read the current instant or the time elapsed since the clock was
/// seeded. The trait is object-safe so the engine can seed any implementation —
/// the real [`MonotonicClock`] or a test mock — and swap it at init without
/// changing [`Ctx`]'s shape.
pub trait Clock: Send + Sync + 'static {
    /// The current instant.
    #[must_use]
    fn now(&self) -> Instant;

    /// The time elapsed since this clock was seeded.
    #[must_use]
    fn elapsed(&self) -> Duration;
}

/// The default [`Clock`]: a real, [`foundation_compact`]-backed monotonic clock.
///
/// On native, `Instant` is `std::time::Instant`; on wasm it is the JS-backed
/// performance clock, so [`Clock::now`] and [`Clock::elapsed`] work across web
/// and non-web endpoints without any platform branch here. `Copy`, no interior
/// mutation — safe to share across threads and tasks.
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

/// Interior of a [`Ctx`]. `Send + Sync` because every field is `Send + Sync`.
pub(crate) struct CtxInner {
    /// General extension point: arbitrary `Arc<T>` entries keyed by `TypeId`.
    map: Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
    /// Engine-seeded standard elements.
    clock: Arc<dyn Clock>,
    config: Arc<Config>,
    rng: Arc<Mutex<ChaCha8Rng>>,
}

impl Ctx {
    /// An empty context: no map entries, a fresh clock, a default config, and a
    /// deterministic default RNG (seed `0`) that the engine may replace at init.
    #[must_use]
    pub fn new() -> Self {
        Ctx(Arc::new(CtxInner {
            map: Mutex::new(HashMap::new()),
            clock: Arc::new(MonotonicClock::new()),
            config: Arc::new(Config::new()),
            rng: Arc::new(Mutex::new(ChaCha8Rng::seed_from_u64(0))),
        }))
    }

    /// Insert `value` under its type, replacing any prior entry and returning it.
    ///
    /// Entries are keyed by `TypeId`, so two distinct `T`s never collide. The
    /// prior entry (if any) is returned for callers that care; like
    /// `HashMap::insert`, ignoring it is fine.
    pub fn insert<T: Any + Send + Sync>(&self, value: Arc<T>) -> Option<Arc<T>> {
        self.0
            .map
            .lock()
            .expect("Ctx map poisoned")
            .insert(TypeId::of::<T>(), value)
            .and_then(|prior| prior.downcast::<T>().ok())
    }

    /// The `Arc<T>` stored under `T`, or `None` if absent or of a different type.
    #[must_use]
    pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.0
            .map
            .lock()
            .expect("Ctx map poisoned")
            .get(&TypeId::of::<T>())
            .cloned()
            .and_then(|value| value.downcast::<T>().ok())
    }

    /// Insert `value` under `T` if absent; return the live `Arc<T>` either way.
    #[must_use]
    pub fn get_or_insert<T: Any + Send + Sync>(&self, value: T) -> Arc<T> {
        let mut map = self.0.map.lock().expect("Ctx map poisoned");
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
    #[must_use]
    pub fn remove<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.0
            .map
            .lock()
            .expect("Ctx map poisoned")
            .remove(&TypeId::of::<T>())
            .and_then(|value| value.downcast::<T>().ok())
    }

    /// Whether a `T` is present in the map.
    #[must_use]
    pub fn contains<T: Any + Send + Sync>(&self) -> bool {
        self.0
            .map
            .lock()
            .expect("Ctx map poisoned")
            .contains_key(&TypeId::of::<T>())
    }

    /// The shared, read-only clock (a `dyn Clock`; the default is
    /// [`MonotonicClock`]).
    #[must_use]
    pub fn clock(&self) -> Arc<dyn Clock> {
        self.0.clock.clone()
    }

    /// The shared, read-only configuration.
    #[must_use]
    pub fn config(&self) -> Arc<Config> {
        self.0.config.clone()
    }

    /// The single shared RNG stream. Lock it to draw: `ctx.rng().lock().next_u64()`.
    ///
    /// No method returns the guard, so a lock can never be held across an
    /// `.await`.
    #[must_use]
    pub fn rng(&self) -> Arc<Mutex<ChaCha8Rng>> {
        self.0.rng.clone()
    }

    /// A new handle over a *copied* map.
    ///
    /// The fresh `HashMap` structurally isolates this handle (insert/remove are
    /// private to it), while the `Arc` values and the standard elements stay
    /// shared — so entries inserted *through* this fork remain reachable from the
    /// parent's `Arc` if the parent already held the same `Arc`.
    #[must_use]
    pub fn fork(&self) -> Ctx {
        let map = self.0.map.lock().expect("Ctx map poisoned").clone();
        Ctx(Arc::new(CtxInner {
            map: Mutex::new(map),
            clock: self.0.clock.clone(),
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

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
    }
}
