//! WASI preview1 host backend: timer-based scheduling and the host clock.
//!
//! The host installs its own [`TimeSource`] with [`WasiHost`] before the module
//! runs. The clock-reading functions ([`register_schedule`], [`tick`],
//! [`time_until_next_event`], [`poll_blocking`] and their callback variants)
//! read the installed source and return [`WasiHostError::TimeSourceNotInstalled`]
//! when the host installed none. The `*_at` variants take the instant from the
//! caller and need no installed source.
//!
//! Issue #219: no direct clock read stays in this module. The host owns the
//! real read in its own implementation of [`TimeSource`].

use alloc::collections::btree_map::BTreeMap;
use alloc::vec::Vec;
use core::fmt;
use foundation_nostd::comp::basic::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::{host_runtime::internal_api, InternalPointer, TickState};

/// A monotonic time source for the WASI host.
///
/// The host installs a source with [`WasiHost`] before the module runs. A test
/// installs a source it moves by hand, so the timer loop is deterministic.
/// The engine's `Clock` sits above this crate, so this crate names its own
/// small trait: a dependency on the engine would be a cycle.
pub trait TimeSource: Send + Sync + 'static {
    /// The current monotonic instant.
    fn now(&self) -> Instant;
}

/// The WASI host environment.
///
/// It owns the [`TimeSource`] that the module's clock-reading functions use.
/// A host builds it with [`WasiHost::with_time_source`] and calls
/// [`WasiHost::install`] once, before it runs the module.
///
/// With no installed source those functions return
/// [`WasiHostError::TimeSourceNotInstalled`]. The module never falls back to a
/// real clock on its own: only the host chooses one.
#[derive(Default)]
pub struct WasiHost {
    time_source: Option<Arc<dyn TimeSource>>,
}

impl WasiHost {
    /// An empty host environment: no time source yet.
    ///
    /// Add one with [`WasiHost::with_time_source`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: use `time_source` for every instant the module reads.
    #[must_use]
    pub fn with_time_source(self, time_source: Arc<dyn TimeSource>) -> Self {
        Self {
            time_source: Some(time_source),
        }
    }

    /// Install this host's time source for the module's clock-reading functions.
    ///
    /// A host calls this once, before it runs the module. A later install
    /// replaces the source; an empty host removes it.
    pub fn install(self) {
        let mut guard = TIME_SOURCE
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        *guard = self.time_source;
    }

    /// Remove the installed source and return it.
    ///
    /// After this call the module's clock-reading functions return
    /// [`WasiHostError::TimeSourceNotInstalled`] again.
    #[must_use]
    pub fn uninstall() -> Option<Arc<dyn TimeSource>> {
        let mut guard = TIME_SOURCE
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        guard.take()
    }
}

/// The WASI host could not read the clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasiHostError {
    /// The host installed no [`TimeSource`] before the module ran.
    TimeSourceNotInstalled,
}

impl fmt::Display for WasiHostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimeSourceNotInstalled => f.write_str("the WASI host installed no time source"),
        }
    }
}

impl std::error::Error for WasiHostError {}

/// The current instant from the installed host time source.
///
/// # Errors
///
/// Returns [`WasiHostError::TimeSourceNotInstalled`] when the host installed no
/// source with [`WasiHost::install`].
pub fn now() -> Result<Instant, WasiHostError> {
    let guard = TIME_SOURCE
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
    guard
        .as_ref()
        .map(|source| source.now())
        .ok_or(WasiHostError::TimeSourceNotInstalled)
}

/// The host-installed time source. The host writes it before the module runs.
static TIME_SOURCE: Mutex<Option<Arc<dyn TimeSource>>> = Mutex::new(None);

struct TimeoutEntry {
    deadline: Instant,
}

struct IntervalEntry {
    period: Duration,
    next_fire: Instant,
}

struct TimerState {
    timeouts: BTreeMap<InternalPointer, TimeoutEntry>,
    intervals: BTreeMap<InternalPointer, IntervalEntry>,
}

impl TimerState {
    const fn new() -> Self {
        Self {
            timeouts: BTreeMap::new(),
            intervals: BTreeMap::new(),
        }
    }
}

static TIMERS: Mutex<TimerState> = Mutex::new(TimerState::new());

static START_TIME: Mutex<Option<Instant>> = Mutex::new(None);

/// Seconds since the first timer call, measured on `now`.
fn monotonic_secs_at(now: Instant) -> f64 {
    let mut guard = START_TIME
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
    let start = *guard.get_or_insert(now);
    now.saturating_duration_since(start).as_secs_f64()
}

/// Register a one-shot timer measured from the installed host instant.
///
/// # Errors
///
/// Returns [`WasiHostError::TimeSourceNotInstalled`] when the host installed no
/// [`TimeSource`]. The callback is not registered in that case.
#[cfg(target_family = "wasm")]
pub fn register_schedule<F>(timing_ms: f64, f: F) -> Result<InternalPointer, WasiHostError>
where
    F: Fn() + 'static,
{
    let now = now()?;
    Ok(arm_schedule_at(
        timing_ms,
        internal_api::register_schedule(f),
        now,
    ))
}

/// Register a one-shot timer measured from the installed host instant.
///
/// # Errors
///
/// Returns [`WasiHostError::TimeSourceNotInstalled`] when the host installed no
/// [`TimeSource`]. The callback is not registered in that case.
#[cfg(not(target_family = "wasm"))]
pub fn register_schedule<F>(timing_ms: f64, f: F) -> Result<InternalPointer, WasiHostError>
where
    F: Fn() + Send + Sync + 'static,
{
    let now = now()?;
    Ok(arm_schedule_at(
        timing_ms,
        internal_api::register_schedule(f),
        now,
    ))
}

/// Run `f` once on the next host tick — the WASI counterpart of the web
/// `register_microtask` (the guest-driven tick loop has no microtask queue, so
/// this is a 0 ms timeout that fires on the next `tick`).
///
/// # Errors
///
/// Returns [`WasiHostError::TimeSourceNotInstalled`] when the host installed no
/// [`TimeSource`].
#[cfg(target_family = "wasm")]
pub fn register_microtask<F>(f: F) -> Result<InternalPointer, WasiHostError>
where
    F: Fn() + 'static,
{
    register_schedule(0.0, f)
}

/// Run `f` once on the next host tick (see the wasm variant).
///
/// # Errors
///
/// Returns [`WasiHostError::TimeSourceNotInstalled`] when the host installed no
/// [`TimeSource`].
#[cfg(not(target_family = "wasm"))]
pub fn register_microtask<F>(f: F) -> Result<InternalPointer, WasiHostError>
where
    F: Fn() + Send + Sync + 'static,
{
    register_schedule(0.0, f)
}

/// Register a task callback as a one-shot timer measured from the installed
/// host instant.
///
/// # Errors
///
/// Returns [`WasiHostError::TimeSourceNotInstalled`] when the host installed no
/// [`TimeSource`]. The callback is not registered in that case.
#[cfg(target_family = "wasm")]
pub fn register_schedule_callback<F>(timing_ms: f64, f: F) -> Result<InternalPointer, WasiHostError>
where
    F: crate::DoTask + 'static,
{
    let now = now()?;
    Ok(arm_schedule_at(
        timing_ms,
        internal_api::register_schedule_callback(f),
        now,
    ))
}

/// Register a task callback as a one-shot timer measured from the installed
/// host instant.
///
/// # Errors
///
/// Returns [`WasiHostError::TimeSourceNotInstalled`] when the host installed no
/// [`TimeSource`]. The callback is not registered in that case.
#[cfg(not(target_family = "wasm"))]
pub fn register_schedule_callback<F>(timing_ms: f64, f: F) -> Result<InternalPointer, WasiHostError>
where
    F: crate::DoTask + Send + Sync + 'static,
{
    let now = now()?;
    Ok(arm_schedule_at(
        timing_ms,
        internal_api::register_schedule_callback(f),
        now,
    ))
}

/// Register a one-shot timer measured from the caller's `now`.
pub fn register_schedule_at<F>(timing_ms: f64, f: F, now: Instant) -> InternalPointer
where
    F: Fn() + Send + Sync + 'static,
{
    arm_schedule_at(timing_ms, internal_api::register_schedule(f), now)
}

/// Arm an already-registered callback as a one-shot timer at `now`.
fn arm_schedule_at(timing_ms: f64, id: InternalPointer, now: Instant) -> InternalPointer {
    let deadline = now + std::time::Duration::from_millis(timing_ms as u64);
    TIMERS
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
        .timeouts
        .insert(id, TimeoutEntry { deadline });
    id
}

pub fn unregister_schedule(id: InternalPointer) {
    TIMERS
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
        .timeouts
        .remove(&id);
    internal_api::unregister_schedule_callback(id);
}

/// Register a recurring timer measured from the installed host instant.
///
/// # Errors
///
/// Returns [`WasiHostError::TimeSourceNotInstalled`] when the host installed no
/// [`TimeSource`]. The callback is not registered in that case.
#[cfg(target_family = "wasm")]
pub fn register_interval<F>(timing_ms: f64, f: F) -> Result<InternalPointer, WasiHostError>
where
    F: Fn() -> TickState + 'static,
{
    let now = now()?;
    Ok(arm_interval_at(
        timing_ms,
        internal_api::register_interval(f),
        now,
    ))
}

/// Register a recurring timer measured from the installed host instant.
///
/// # Errors
///
/// Returns [`WasiHostError::TimeSourceNotInstalled`] when the host installed no
/// [`TimeSource`]. The callback is not registered in that case.
#[cfg(not(target_family = "wasm"))]
pub fn register_interval<F>(timing_ms: f64, f: F) -> Result<InternalPointer, WasiHostError>
where
    F: Fn() -> TickState + Send + Sync + 'static,
{
    let now = now()?;
    Ok(arm_interval_at(
        timing_ms,
        internal_api::register_interval(f),
        now,
    ))
}

/// Register an interval callback measured from the installed host instant.
///
/// # Errors
///
/// Returns [`WasiHostError::TimeSourceNotInstalled`] when the host installed no
/// [`TimeSource`]. The callback is not registered in that case.
#[cfg(target_family = "wasm")]
pub fn register_interval_callback<F>(timing_ms: f64, f: F) -> Result<InternalPointer, WasiHostError>
where
    F: crate::IntervalCallback + 'static,
{
    let now = now()?;
    Ok(arm_interval_at(
        timing_ms,
        internal_api::register_interval_callback(f),
        now,
    ))
}

/// Register an interval callback measured from the installed host instant.
///
/// # Errors
///
/// Returns [`WasiHostError::TimeSourceNotInstalled`] when the host installed no
/// [`TimeSource`]. The callback is not registered in that case.
#[cfg(not(target_family = "wasm"))]
pub fn register_interval_callback<F>(timing_ms: f64, f: F) -> Result<InternalPointer, WasiHostError>
where
    F: crate::IntervalCallback + Send + Sync + 'static,
{
    let now = now()?;
    Ok(arm_interval_at(
        timing_ms,
        internal_api::register_interval_callback(f),
        now,
    ))
}

/// Register a recurring timer measured from the caller's `now`.
fn arm_interval_at(timing_ms: f64, id: InternalPointer, now: Instant) -> InternalPointer {
    let period = Duration::from_millis(timing_ms as u64);
    TIMERS
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
        .intervals
        .insert(
            id,
            IntervalEntry {
                period,
                next_fire: now + period,
            },
        );
    id
}

pub fn unregister_interval(id: InternalPointer) {
    TIMERS
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
        .intervals
        .remove(&id);
    internal_api::unregister_interval_callback(id);
}

/// Advance the timer loop: fire expired one-shot timeouts, due recurring
/// intervals, and animation-frame callbacks. Returns `REQUEUE` if pending
/// work remains, `STOP` when idle.
///
/// The WASI host (or the guest's own main loop) should call this
/// periodically. For blocking operation, use [`poll_blocking`] instead.
///
/// # Errors
///
/// Returns [`WasiHostError::TimeSourceNotInstalled`] when the host installed no
/// [`TimeSource`].
pub fn tick() -> Result<TickState, WasiHostError> {
    Ok(tick_at(now()?))
}

/// Fire the timers that are due at `now`.
pub fn tick_at(now: Instant) -> TickState {
    // -- One-shot timeouts --
    // Collect expired IDs under the lock, then drop before firing callbacks
    // (callbacks may re-register timers, which would deadlock if held).
    let expired: Vec<InternalPointer> = {
        let timers = TIMERS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        timers
            .timeouts
            .iter()
            .filter(|(_, e)| now >= e.deadline)
            .map(|(id, _)| *id)
            .collect()
    };

    for id in &expired {
        let _ = internal_api::run_schedule_callback(*id);
    }

    if !expired.is_empty() {
        let mut timers = TIMERS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        for id in &expired {
            timers.timeouts.remove(id);
        }
        drop(timers);
        for id in &expired {
            internal_api::unregister_schedule_callback(*id);
        }
    }

    // -- Recurring intervals --
    let due: Vec<InternalPointer> = {
        let timers = TIMERS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        timers
            .intervals
            .iter()
            .filter(|(_, e)| now >= e.next_fire)
            .map(|(id, _)| *id)
            .collect()
    };

    let mut stopped = Vec::new();
    for id in &due {
        if let Ok(TickState::STOP) = internal_api::run_interval_callback(*id) {
            stopped.push(*id);
        }
    }

    {
        let mut timers = TIMERS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        for id in &due {
            if stopped.contains(id) {
                timers.intervals.remove(id);
            } else if let Some(entry) = timers.intervals.get_mut(id) {
                entry.next_fire = now + entry.period;
            }
        }
    }

    for id in &stopped {
        internal_api::unregister_interval_callback(*id);
    }

    // -- Animation frames (monotonic timestamp in seconds) --
    internal_api::run_animation_frames(monotonic_secs_at(now));

    // -- Check if work remains --
    let timers = TIMERS
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
    let anim_count = internal_api::get_total_animation_callbacks();
    if timers.timeouts.is_empty() && timers.intervals.is_empty() && anim_count == 0 {
        TickState::STOP
    } else {
        TickState::REQUEUE
    }
}

/// Returns the duration until the next timer fires, or `None` when idle.
/// The WASI host can use this to sleep efficiently between ticks.
///
/// # Errors
///
/// Returns [`WasiHostError::TimeSourceNotInstalled`] when the host installed no
/// [`TimeSource`].
pub fn time_until_next_event() -> Result<Option<Duration>, WasiHostError> {
    Ok(time_until_next_event_at(now()?))
}

/// The duration until the next timer fires at `now`, or `None` when idle.
pub fn time_until_next_event_at(now: Instant) -> Option<Duration> {
    let timers = TIMERS
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);

    let mut earliest: Option<Instant> = None;

    for entry in timers.timeouts.values() {
        if earliest.is_none_or(|e| entry.deadline < e) {
            earliest = Some(entry.deadline);
        }
    }

    for entry in timers.intervals.values() {
        if earliest.is_none_or(|e| entry.next_fire < e) {
            earliest = Some(entry.next_fire);
        }
    }

    earliest.map(|e| e.saturating_duration_since(now))
}

/// Sleep until the next timer event, then fire it. Combines
/// [`time_until_next_event`] + `std::thread::sleep` + [`tick`] into one
/// call — the simplest WASI event loop: `loop { poll_blocking(); }`.
///
/// # Errors
///
/// Returns [`WasiHostError::TimeSourceNotInstalled`] when the host installed no
/// [`TimeSource`].
pub fn poll_blocking() -> Result<TickState, WasiHostError> {
    if let Some(wait) = time_until_next_event()? {
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
    }
    tick()
}

/// Returns `true` if there are any pending timeouts, active intervals, or
/// registered animation-frame callbacks.
pub fn has_pending_work() -> bool {
    let timers = TIMERS
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
    let anim_count = internal_api::get_total_animation_callbacks();
    !timers.timeouts.is_empty() || !timers.intervals.is_empty() || anim_count > 0
}
