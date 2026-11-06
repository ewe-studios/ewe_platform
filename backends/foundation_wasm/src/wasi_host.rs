use alloc::collections::btree_map::BTreeMap;
use alloc::vec::Vec;
use foundation_nostd::comp::basic::Mutex;

use crate::{host_runtime::internal_api, InternalPointer, TickState};

struct TimeoutEntry {
    deadline: std::time::Instant,
}

struct IntervalEntry {
    period: std::time::Duration,
    next_fire: std::time::Instant,
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

static START_TIME: Mutex<Option<std::time::Instant>> = Mutex::new(None);

/// The host's monotonic time.
///
/// The WASI extern-C ABI has no Rust caller to pass a clock, so this is the one
/// place the real monotonic read stays. Every timer function also has an `_at`
/// variant that takes the instant from its caller; a host or a test that has a
/// clock uses that variant.
fn host_now() -> std::time::Instant {
    std::time::Instant::now() // time-source: the WASI extern-C ABI has no Rust caller to pass a clock
}

/// Seconds since the first timer call, measured on `now`.
fn monotonic_secs_at(now: std::time::Instant) -> f64 {
    let mut guard = START_TIME
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
    let start = *guard.get_or_insert(now);
    now.saturating_duration_since(start).as_secs_f64()
}

#[cfg(target_family = "wasm")]
pub fn register_schedule<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: Fn() + 'static,
{
    arm_schedule_at(timing_ms, internal_api::register_schedule(f), host_now())
}

#[cfg(not(target_family = "wasm"))]
pub fn register_schedule<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: Fn() + Send + Sync + 'static,
{
    arm_schedule_at(timing_ms, internal_api::register_schedule(f), host_now())
}

/// Run `f` once on the next host tick — the WASI counterpart of the web
/// `register_microtask` (the guest-driven tick loop has no microtask queue, so
/// this is a 0 ms timeout that fires on the next `tick`).
#[cfg(target_family = "wasm")]
pub fn register_microtask<F>(f: F) -> InternalPointer
where
    F: Fn() + 'static,
{
    register_schedule(0.0, f)
}

/// Run `f` once on the next host tick (see the wasm variant).
#[cfg(not(target_family = "wasm"))]
pub fn register_microtask<F>(f: F) -> InternalPointer
where
    F: Fn() + Send + Sync + 'static,
{
    register_schedule(0.0, f)
}

#[cfg(target_family = "wasm")]
pub fn register_schedule_callback<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: crate::DoTask + 'static,
{
    arm_schedule_at(
        timing_ms,
        internal_api::register_schedule_callback(f),
        host_now(),
    )
}

#[cfg(not(target_family = "wasm"))]
pub fn register_schedule_callback<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: crate::DoTask + Send + Sync + 'static,
{
    arm_schedule_at(
        timing_ms,
        internal_api::register_schedule_callback(f),
        host_now(),
    )
}

/// Register a one-shot timer measured from the caller's `now`.
pub fn register_schedule_at<F>(timing_ms: f64, f: F, now: std::time::Instant) -> InternalPointer
where
    F: Fn() + Send + Sync + 'static,
{
    arm_schedule_at(timing_ms, internal_api::register_schedule(f), now)
}

/// Arm an already-registered callback as a one-shot timer at `now`.
fn arm_schedule_at(
    timing_ms: f64,
    id: InternalPointer,
    now: std::time::Instant,
) -> InternalPointer {
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

#[cfg(target_family = "wasm")]
pub fn register_interval<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: Fn() -> TickState + 'static,
{
    arm_interval_at(timing_ms, internal_api::register_interval(f), host_now())
}

#[cfg(not(target_family = "wasm"))]
pub fn register_interval<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: Fn() -> TickState + Send + Sync + 'static,
{
    arm_interval_at(timing_ms, internal_api::register_interval(f), host_now())
}

#[cfg(target_family = "wasm")]
pub fn register_interval_callback<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: crate::IntervalCallback + 'static,
{
    arm_interval_at(
        timing_ms,
        internal_api::register_interval_callback(f),
        host_now(),
    )
}

#[cfg(not(target_family = "wasm"))]
pub fn register_interval_callback<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: crate::IntervalCallback + Send + Sync + 'static,
{
    arm_interval_at(
        timing_ms,
        internal_api::register_interval_callback(f),
        host_now(),
    )
}

/// Register a recurring timer measured from the caller's `now`.
fn arm_interval_at(
    timing_ms: f64,
    id: InternalPointer,
    now: std::time::Instant,
) -> InternalPointer {
    let period = std::time::Duration::from_millis(timing_ms as u64);
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
pub fn tick() -> TickState {
    tick_at(host_now())
}

/// Fire the timers that are due at `now`.
pub fn tick_at(now: std::time::Instant) -> TickState {
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
pub fn time_until_next_event() -> Option<std::time::Duration> {
    time_until_next_event_at(host_now())
}

/// The duration until the next timer fires at `now`, or `None` when idle.
pub fn time_until_next_event_at(now: std::time::Instant) -> Option<std::time::Duration> {
    let timers = TIMERS
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);

    let mut earliest: Option<std::time::Instant> = None;

    for entry in timers.timeouts.values() {
        if earliest.map_or(true, |e| entry.deadline < e) {
            earliest = Some(entry.deadline);
        }
    }

    for entry in timers.intervals.values() {
        if earliest.map_or(true, |e| entry.next_fire < e) {
            earliest = Some(entry.next_fire);
        }
    }

    earliest.map(|e| e.saturating_duration_since(now))
}

/// Sleep until the next timer event, then fire it. Combines
/// [`time_until_next_event`] + `std::thread::sleep` + [`tick`] into one
/// call — the simplest WASI event loop: `loop { poll_blocking(); }`.
pub fn poll_blocking() -> TickState {
    if let Some(wait) = time_until_next_event() {
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
