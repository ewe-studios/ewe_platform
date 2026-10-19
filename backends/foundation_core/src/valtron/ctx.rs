//! Engine-provided, per-poll context handed to every task (feature 04-ctx).
//!
//! [`Ctx`] is a cloneable `Send + Sync` handle over a [`CtxInner`]: an
//! interior-mutable type map keyed by [`TypeId`], plus three engine-seeded
//! standard elements — a [`Clock`], a [`Config`], and a shared [`ChaCha8Rng`].
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

/// A read-only monotonic clock, seeded by the engine and shared as an `Arc`.
///
/// Tasks read the current instant or the time elapsed since the clock was
/// created. `Clock` is `Copy` and has no interior mutation — it is safe to share
/// across threads and tasks.
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    origin: Instant,
}

// TODO: does this take into account wasm for web and non-web endpoints,
// there should be a crate to make these easily work across already in the backends/ find it and correctly use it, more so, the clock should probably be meeting a trait instead, so that its easy for us to swap in a mock for its usage.
impl Clock {
    /// A clock whose origin is "now".
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }

    /// The current instant.
    #[must_use]
    pub fn now(&self) -> Instant {
        Instant::now()
    }

    /// The time elapsed since this clock was created.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.origin.elapsed()
    }
}

impl Default for Clock {
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
    clock: Arc<Clock>,
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
            clock: Arc::new(Clock::new()),
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

    /// The shared, read-only clock.
    #[must_use]
    pub fn clock(&self) -> Arc<Clock> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use foundation_compact::rng::Rng;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Compile-time assertion that `Ctx` is usable across threads (and on the
    /// single-threaded engine). Fails to compile if `Ctx` stops being `Send + Sync`.
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_send_sync_impl<T: Send + Sync>() {}

    #[test]
    fn ctx_is_send_and_sync() {
        assert_send_sync::<Ctx>();
        assert_send_sync_impl::<Clock>();
        assert_send_sync_impl::<Config>();
    }

    #[test]
    fn empty_ctx_has_no_entries() {
        let ctx = Ctx::new();
        assert!(!ctx.contains::<u32>());
        assert!(ctx.get::<u32>().is_none());
        assert!(ctx.remove::<u32>().is_none());
    }

    #[test]
    fn map_round_trip() {
        let ctx = Ctx::new();
        assert!(ctx.insert(Arc::new(42u32)).is_none());

        let got = ctx
            .get::<u32>()
            .expect("inserted value should be reachable");
        assert_eq!(*got, 42);
        assert!(ctx.contains::<u32>());

        let removed = ctx.remove::<u32>().expect("remove should return the value");
        assert_eq!(*removed, 42);
        assert!(!ctx.contains::<u32>());
        assert!(ctx.get::<u32>().is_none());
    }

    #[test]
    fn insert_replaces_and_returns_prior() {
        let ctx = Ctx::new();
        let prior = ctx.insert(Arc::new(1u32));
        assert!(prior.is_none(), "first insert has no prior");

        let prior = ctx.insert(Arc::new(2u32));
        assert_eq!(
            prior.map(|v| *v),
            Some(1),
            "replace returns the previous Arc"
        );
        assert_eq!(*ctx.get::<u32>().unwrap(), 2);
    }

    #[test]
    fn get_with_wrong_type_returns_none_not_panic() {
        let ctx = Ctx::new();
        ctx.insert(Arc::new(1u32));
        // Same key domain, different type — must be `None`, never a downcast panic.
        assert!(ctx.get::<u64>().is_none());
        assert!(ctx.get::<i32>().is_none());
        assert!(ctx.remove::<u64>().is_none());
        assert!(!ctx.contains::<u64>());
    }

    #[test]
    fn get_or_insert_returns_live_arc_and_dedupes() {
        let ctx = Ctx::new();
        let a = ctx.get_or_insert(7u32);
        let b = ctx.get_or_insert(7u32);
        assert_eq!(*a, 7);
        assert!(
            Arc::ptr_eq(&a, &b),
            "get_or_insert must return the existing Arc, not re-insert"
        );
    }

    #[test]
    fn clone_shares_the_map() {
        let ctx = Ctx::new();
        let shared = ctx.clone();

        ctx.insert(Arc::new(99u32));
        // The clone observes the parent's write — same underlying map.
        assert_eq!(*shared.get::<u32>().unwrap(), 99);

        shared.insert(Arc::new(100u32));
        // And vice versa: the parent observes the clone's write.
        assert_eq!(*ctx.get::<u32>().unwrap(), 100);
    }

    #[test]
    fn fork_isolates_the_map() {
        let ctx = Ctx::new();
        let fork = ctx.fork();

        ctx.insert(Arc::new(1u32));
        // The fork has its own (copied) map: the parent's insert is invisible.
        assert!(
            !fork.contains::<u32>(),
            "a forked map must not see the parent's insert"
        );

        fork.insert(Arc::new(2u32));
        // The parent's map is unaffected by the fork's insert: still holds value 1.
        assert_eq!(
            *ctx.get::<u32>().unwrap(),
            1,
            "the parent must not see the fork's insert"
        );
        assert_eq!(*fork.get::<u32>().unwrap(), 2);
    }

    #[test]
    fn fork_shares_arc_values() {
        let ctx = Ctx::new();
        let counter = Arc::new(AtomicUsize::new(0));
        ctx.insert(counter.clone());

        let fork = ctx.fork();
        // The map structure is copied, but the `Arc` value is shared.
        fork.get::<AtomicUsize>()
            .expect("forked map retains the Arc entry")
            .fetch_add(1, Ordering::SeqCst);

        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "mutating the shared Arc through the fork is visible through the parent"
        );
    }

    #[test]
    fn standard_elements_are_shared_across_forks() {
        let ctx = Ctx::new();
        let fork = ctx.fork();

        assert!(Arc::ptr_eq(&ctx.clock(), &fork.clock()));
        assert!(Arc::ptr_eq(&ctx.config(), &fork.config()));
        assert!(Arc::ptr_eq(&ctx.rng(), &fork.rng()));
    }

    #[test]
    fn standard_elements_are_reachable_and_rng_draws() {
        let ctx = Ctx::new();

        // Clock reads are live (elapsed grows, or at least is a valid Duration).
        let _ = ctx.clock().now();
        let _ = ctx.clock().elapsed();
        let _ = ctx.config();

        // The RNG is a real, working stream.
        let rng = ctx.rng();
        let mut guard = rng.lock().expect("rng lock poisoned");
        let first = guard.next_u64();
        let second = guard.next_u64();
        assert_ne!(
            first, second,
            "two draws from a live RNG stream must differ"
        );
    }

    #[test]
    fn fork_preserves_shared_rng_stream_state() {
        // The RNG is shared (same Arc) across a fork, so a draw through one
        // handle advances the stream seen through the other.
        let ctx = Ctx::new();
        let fork = ctx.fork();

        let a = ctx.rng().lock().expect("lock").next_u64();
        let b = fork.rng().lock().expect("lock").next_u64();
        assert_ne!(a, b, "shared RNG advances across fork handles");
    }
}
