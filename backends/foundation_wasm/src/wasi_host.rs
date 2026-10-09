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

fn monotonic_secs() -> f64 {
    let mut guard = START_TIME
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
    let start = *guard.get_or_insert_with(std::time::Instant::now);
    start.elapsed().as_secs_f64()
}

#[cfg(target_family = "wasm")]
pub fn register_schedule<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: Fn() + 'static,
{
    register_schedule_inner(timing_ms, internal_api::register_schedule(f))
}

#[cfg(not(target_family = "wasm"))]
pub fn register_schedule<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: Fn() + Send + Sync + 'static,
{
    register_schedule_inner(timing_ms, internal_api::register_schedule(f))
}

#[cfg(target_family = "wasm")]
pub fn register_schedule_callback<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: crate::DoTask + 'static,
{
    register_schedule_inner(timing_ms, internal_api::register_schedule_callback(f))
}

#[cfg(not(target_family = "wasm"))]
pub fn register_schedule_callback<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: crate::DoTask + Send + Sync + 'static,
{
    register_schedule_inner(timing_ms, internal_api::register_schedule_callback(f))
}

fn register_schedule_inner(timing_ms: f64, id: InternalPointer) -> InternalPointer {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_millis(timing_ms as u64);
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
    register_interval_inner(timing_ms, internal_api::register_interval(f))
}

#[cfg(not(target_family = "wasm"))]
pub fn register_interval<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: Fn() -> TickState + Send + Sync + 'static,
{
    register_interval_inner(timing_ms, internal_api::register_interval(f))
}

#[cfg(target_family = "wasm")]
pub fn register_interval_callback<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: crate::IntervalCallback + 'static,
{
    register_interval_inner(timing_ms, internal_api::register_interval_callback(f))
}

#[cfg(not(target_family = "wasm"))]
pub fn register_interval_callback<F>(timing_ms: f64, f: F) -> InternalPointer
where
    F: crate::IntervalCallback + Send + Sync + 'static,
{
    register_interval_inner(timing_ms, internal_api::register_interval_callback(f))
}

fn register_interval_inner(timing_ms: f64, id: InternalPointer) -> InternalPointer {
    let period = std::time::Duration::from_millis(timing_ms as u64);
    TIMERS
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
        .intervals
        .insert(
            id,
            IntervalEntry {
                period,
                next_fire: std::time::Instant::now() + period,
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
    let now = std::time::Instant::now();

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
    let now = std::time::Instant::now();
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
        let now = std::time::Instant::now();
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
    internal_api::run_animation_frames(monotonic_secs());

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
    let now = std::time::Instant::now();
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
