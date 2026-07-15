//! Runtime backend state and the load balancer.
//!
//! WHY: `BackendTarget` is static config. At runtime a backend also has a
//! lifecycle state (`Active`/`Draining`/`Paused`), a health flag the probes
//! flip, and an in-flight request refcount used both for `max_connections`
//! enforcement now and connection draining in stage 3. The load balancer picks
//! among only the *eligible* backends (active, healthy, under their connection
//! cap) using weighted round-robin.
//!
//! WHAT: [`BackendRuntime`] (one per configured backend), [`BackendLease`] (an
//! RAII in-flight guard), and [`ServiceRuntime`] which owns a service's backends
//! and performs weighted round-robin selection.
//!
//! HOW: Selection is smooth weighted round-robin (the nginx algorithm) run under
//! a per-service mutex so concurrent forwarder threads see a consistent
//! rotation. Eligibility and the in-flight count are plain atomics.

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::config::{BackendProtocol, BackendState, BackendTarget, ServiceConfig};

/// Runtime state for a single backend target.
#[derive(Debug)]
pub struct BackendRuntime {
    target: BackendTarget,
    state: Mutex<BackendState>,
    healthy: AtomicBool,
    inflight: AtomicU32,
    /// Smooth-weighted-round-robin running weight. Mutated only while the owning
    /// `ServiceRuntime`'s selection mutex is held.
    current_weight: AtomicI64,
}

impl BackendRuntime {
    /// Create runtime state for `target`.
    ///
    /// A backend starts `Active` and is assumed healthy until the first probe
    /// says otherwise — a service with no health check configured is therefore
    /// always in rotation.
    #[must_use]
    pub fn new(target: BackendTarget) -> Self {
        Self {
            target,
            state: Mutex::new(BackendState::Active),
            healthy: AtomicBool::new(true),
            inflight: AtomicU32::new(0),
            current_weight: AtomicI64::new(0),
        }
    }

    #[must_use]
    pub fn target(&self) -> &BackendTarget {
        &self.target
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.target.url
    }

    #[must_use]
    pub fn protocol(&self) -> BackendProtocol {
        self.target.protocol()
    }

    #[must_use]
    pub fn state(&self) -> BackendState {
        *self.state.lock().expect("backend state mutex poisoned")
    }

    /// Set the lifecycle state (used by drain/pause control and tests).
    pub fn set_state(&self, state: BackendState) {
        *self.state.lock().expect("backend state mutex poisoned") = state;
    }

    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::SeqCst)
    }

    /// Set the health flag. Called by the health probe state machine.
    pub fn set_healthy(&self, healthy: bool) {
        self.healthy.store(healthy, Ordering::SeqCst);
    }

    /// Current number of in-flight requests routed to this backend.
    #[must_use]
    pub fn inflight(&self) -> u32 {
        self.inflight.load(Ordering::SeqCst)
    }

    /// Whether this backend may receive a new request right now.
    ///
    /// Eligible = `Active` **and** healthy **and** under its `max_connections`
    /// cap. `Draining`/`Paused` backends never receive new traffic.
    #[must_use]
    pub fn is_eligible(&self) -> bool {
        self.state() == BackendState::Active
            && self.is_healthy()
            && self.inflight() < self.target.max_connections
    }

    /// Try to reserve one in-flight slot, returning a lease on success.
    ///
    /// HOW: A CAS loop increments `inflight` only while it stays below
    /// `max_connections`, so two racing forwarders can never overshoot the cap.
    fn try_reserve(self: &Arc<Self>) -> Option<BackendLease> {
        let cap = self.target.max_connections;
        loop {
            let current = self.inflight.load(Ordering::SeqCst);
            if current >= cap {
                return None;
            }
            if self
                .inflight
                .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return Some(BackendLease {
                    backend: Arc::clone(self),
                });
            }
        }
    }
}

/// RAII guard for one in-flight request against a backend.
///
/// Holds the backend's in-flight count up by one for the lifetime of the guard;
/// dropping it (when the forwarder thread finishes) decrements the count.
#[derive(Debug)]
pub struct BackendLease {
    backend: Arc<BackendRuntime>,
}

impl BackendLease {
    #[must_use]
    pub fn backend(&self) -> &Arc<BackendRuntime> {
        &self.backend
    }
}

impl Drop for BackendLease {
    fn drop(&mut self) {
        // Saturating decrement: never wrap below zero even if mis-paired.
        let prev = self.backend.inflight.fetch_sub(1, Ordering::SeqCst);
        if prev == 0 {
            // Restore — this should be impossible, but a wrapped counter would
            // permanently wedge the backend out of rotation.
            self.backend.inflight.store(0, Ordering::SeqCst);
            tracing::error!(url = %self.backend.url(), "backend lease dropped with zero in-flight");
        }
    }
}

/// A service's backends plus its weighted-round-robin selection state.
#[derive(Debug)]
pub struct ServiceRuntime {
    config: ServiceConfig,
    backends: Vec<Arc<BackendRuntime>>,
    /// Serializes smooth-weighted-round-robin selection across forwarder threads.
    select_lock: Mutex<()>,
}

impl ServiceRuntime {
    /// Build runtime state for `config`, one [`BackendRuntime`] per target.
    #[must_use]
    pub fn new(config: ServiceConfig) -> Self {
        let backends = config
            .backends
            .iter()
            .cloned()
            .map(|t| Arc::new(BackendRuntime::new(t)))
            .collect();
        Self {
            config,
            backends,
            select_lock: Mutex::new(()),
        }
    }

    #[must_use]
    pub fn config(&self) -> &ServiceConfig {
        &self.config
    }

    #[must_use]
    pub fn backends(&self) -> &[Arc<BackendRuntime>] {
        &self.backends
    }

    /// Pick the next eligible backend and reserve an in-flight slot on it.
    ///
    /// Returns `None` when no backend is `Active`, healthy, and under its
    /// connection cap — the caller answers `503` in that case.
    ///
    /// HOW: Smooth weighted round-robin over the eligible subset. Each call adds
    /// every eligible backend's weight to its running `current_weight`, selects
    /// the maximum, then subtracts the total eligible weight from the winner.
    /// Over a cycle this distributes requests in proportion to weight while
    /// interleaving them smoothly rather than in bursts.
    #[must_use]
    pub fn pick(&self) -> Option<BackendLease> {
        let _guard = self.select_lock.lock().expect("select mutex poisoned");

        let mut total: i64 = 0;
        let mut best: Option<&Arc<BackendRuntime>> = None;
        let mut best_weight = i64::MIN;

        for backend in &self.backends {
            if !backend.is_eligible() {
                continue;
            }
            let weight = i64::from(backend.target().weight.max(1));
            total += weight;
            let updated = backend.current_weight.fetch_add(weight, Ordering::SeqCst) + weight;
            if updated > best_weight {
                best_weight = updated;
                best = Some(backend);
            }
        }

        let chosen = best?;
        chosen.current_weight.fetch_sub(total, Ordering::SeqCst);

        // Reserve on the winner. If the reservation loses a capacity race (the
        // last slot was taken between eligibility and reservation), fall back to
        // any other backend that still has room.
        if let Some(lease) = chosen.try_reserve() {
            return Some(lease);
        }
        self.backends
            .iter()
            .find_map(|b| if b.is_eligible() { b.try_reserve() } else { None })
    }
}
