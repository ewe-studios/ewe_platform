//! The activity indicator measures elapsed time on the injected clock
//! (spec 70 feature 134).
//!
//! WHY: the animation thread read the real monotonic clock, so a deterministic
//! test could not state the elapsed time it showed.
//!
//! WHAT: with a `SimulatedClock`, an elapsed readout of five seconds appears
//! without a five-second wait.
//!
//! HOW: before this feature the animation started from `Instant::now()` and
//! moved only with real time.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::{Arc, Mutex};
use std::time::Duration;

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

#[test]
fn the_activity_elapsed_follows_the_injected_clock() {
    let recorder = Recorder::default();
    let display: SharedDisplay = Arc::new(Mutex::new(Box::new(recorder.clone())));

    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let activity = Activity::start(
        display,
        ActivityStyle {
            show_elapsed: true,
            ..ActivityStyle::default()
        },
        "working".to_string(),
        Arc::clone(&clock) as Arc<dyn foundation_core::valtron::Clock>,
    );

    clock.advance(Duration::from_secs(5));

    // Wait for one animation frame to draw. This is a real wait for a frame,
    // not a wait that proves a timeout.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let elapsed = recorder.elapsed.lock().expect("elapsed recorder").clone();
        if elapsed.contains(&Some(Duration::from_secs(5))) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the activity never reported the simulated five seconds; saw {elapsed:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    drop(activity);
}
