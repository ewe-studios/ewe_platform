//! The activity indicator measures elapsed time on the clock of its `Ctx`
//! (spec 70 feature 134, spec 76 feature 05).
//!
//! WHY: the animation thread read the real monotonic clock, so a deterministic
//! test could not state the elapsed time it showed.
//!
//! WHAT: with a `SimulatedClock` inside a `Ctx`, an elapsed readout of five
//! seconds appears without a five-second wait. A second test proves that the
//! indicator reads the clock of the `Ctx` it is given, and no other clock.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron::SimulatedClock;
use foundation_repl::{
    Activity, ActivityStyle, ActivityView, InputView, ReplDisplay, SharedDisplay,
};

/// A renderer that records every elapsed value it is asked to draw.
#[derive(Clone, Default)]
struct Recorder {
    elapsed: Arc<Mutex<Vec<Option<Duration>>>>,
}

impl ReplDisplay for Recorder {
    fn print_banner(&mut self, _banner: &str) {}
    fn render_input(&mut self, _view: &InputView<'_>) {}
    fn finish_input(&mut self, _view: &InputView<'_>) {}
    fn print_response(&mut self, _response: &str) {}
    fn print_error(&mut self, _msg: &str) {}
    fn render_activity(&mut self, view: &ActivityView<'_>) {
        self.elapsed
            .lock()
            .expect("elapsed recorder")
            .push(view.elapsed);
    }
    fn stream_push(&mut self, _text: &str) {}
    fn end_activity(&mut self) {}
    fn clear_screen(&mut self) {}
    fn shutdown(&mut self) {}
}

fn elapsed_style() -> ActivityStyle {
    ActivityStyle {
        show_elapsed: true,
        ..ActivityStyle::default()
    }
}

/// Wait until the renderer has drawn `expected`.
///
/// WHY: the animation runs on its own thread, so the test must wait for a
/// frame. The wait is bounded, so a missing frame fails the test.
fn wait_for_elapsed(recorder: &Recorder, expected: Duration) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let elapsed = recorder.elapsed.lock().expect("elapsed recorder").clone();
        if elapsed.contains(&Some(expected)) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the activity never reported {expected:?}; saw {elapsed:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_activity_elapsed_follows_the_injected_clock() {
    let recorder = Recorder::default();
    let display: SharedDisplay = Arc::new(Mutex::new(Box::new(recorder.clone())));

    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let activity = Activity::start(
        display,
        elapsed_style(),
        "working".to_string(),
        Ctx::fixed(clock.clone()),
    );

    clock.advance(Duration::from_secs(5));
    wait_for_elapsed(&recorder, Duration::from_secs(5));

    drop(activity);
}

/// WHY: a `Ctx` can carry a different clock than the one a caller kept. The
/// indicator must follow the clock inside the `Ctx` it received.
/// WHAT: the `Ctx` is rebased onto a second clock. Only that second clock
/// moves, and the indicator shows its elapsed time.
#[test]
fn the_activity_reads_the_clock_of_the_ctx_it_is_given() {
    let recorder = Recorder::default();
    let display: SharedDisplay = Arc::new(Mutex::new(Box::new(recorder.clone())));

    let unused_clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let used_clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(unused_clock.clone()).with_clock(used_clock.clone());

    let activity = Activity::start(display, elapsed_style(), "working".to_string(), ctx);

    unused_clock.advance(Duration::from_secs(30));
    used_clock.advance(Duration::from_secs(2));
    wait_for_elapsed(&recorder, Duration::from_secs(2));

    drop(activity);
}
