//! The process-wide timer thread, re-exported from `foundation_context`.
//!
//! WHY: [`Ctx::timer`] must derive a timer from the context clock, so the
//! implementation sits below `foundation_core`. The code moved to
//! [`foundation_context::timer`]; this module re-exports every item, so an
//! existing path such as `foundation_core::valtron::timer::sleep_for` keeps
//! working.
//!
//! WHAT: [`sleep_until`] / [`sleep_for`], futures that complete at a deadline
//! without any valtron task; [`call_at`] for a one-shot callback on the timer
//! thread; and [`ContextTimer`], the default timer of a context.

pub use foundation_context::timer::{
    call_at, notify_clock_advanced, sleep_for, sleep_until, ContextTimer, Sleep, TimerCall,
};
