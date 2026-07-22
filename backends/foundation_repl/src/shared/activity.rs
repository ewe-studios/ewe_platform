//! The "something is happening" indicator, and the streaming sink attached to it.
//!
//! WHY: between submitting a message and the reply appearing, the REPL has
//! nothing to say — and a terminal showing nothing is indistinguishable from
//! one that has hung. The caller is usually blocked inside a model call, so it
//! cannot repaint anything itself. Worse, a reply that took ten seconds to
//! generate should not appear all at once at the end when it was available a
//! token at a time.
//!
//! WHAT: [`Activity`] is a guard. Take one before starting slow work and an
//! animated status box replaces the input box; push text into it as results
//! arrive and that text streams into the output region above; drop it and the
//! input box comes back.
//!
//! HOW: a background thread advances the animation, drawing through the same
//! lock every other part of the REPL renders through — so a spinner frame can
//! never interleave with text being printed. Streamed text is wrapped as it
//! arrives; rows that are full can no longer change, so they are committed to
//! the terminal's scrollback and only the unfinished tail stays live. Dropping
//! the guard stops the thread, flushes that tail and clears the status box.

use core::fmt;
use core::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use crate::shared::theme::ActivityStyle;
use crate::shared::traits::SharedDisplay;

/// What the indicator should show at this instant.
///
/// WHY: passing frame, label, elapsed and progress as four parameters made the
/// display trait awkward to implement and easy to call wrongly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActivityView<'a> {
    /// The animation frame to draw, or empty when the animation is disabled.
    pub frame: &'a str,
    /// Text describing the work in progress.
    pub label: &'a str,
    /// How long the work has been running, when the style asks for it.
    pub elapsed: Option<Duration>,
    /// Completion between `0.0` and `1.0`, when the caller knows it.
    ///
    /// `Some` switches the indicator from a spinner to a progress bar.
    pub progress: Option<f64>,
}

impl fmt::Display for ActivityView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.frame, self.label)?;
        if let Some(progress) = self.progress {
            write!(f, " {:.0}%", progress * 100.0)?;
        }
        if let Some(elapsed) = self.elapsed {
            write!(f, " {:.1}s", elapsed.as_secs_f64())?;
        }
        Ok(())
    }
}

/// State the animation thread and the caller both touch.
#[derive(Debug)]
struct ActivityState {
    label: String,
    progress: Option<f64>,
}

/// A running activity indicator, and the sink for anything it streams.
///
/// WHY: work can end by returning, by erroring, or by unwinding, and the
/// indicator has to come down in all three cases — which is what a guard does
/// and an explicit stop call does not.
///
/// WHAT: holds the animation thread. The label and progress can be changed
/// while it runs, and [`Activity::push`] streams partial results into the
/// output region above the indicator as they arrive.
///
/// HOW: take one from [`Repl::animation`](crate::Repl::animation) and let it
/// drop, or end it early with [`Activity::finish`].
///
/// # Examples
///
/// Streaming a reply as it is generated:
///
/// ```no_run
/// use foundation_repl::Repl;
///
/// let repl = Repl::new();
/// for input in repl.messages() {
///     let stream = repl.animation("thinking");
///     for token in generate(&input) {
///         stream.push(&token);
///     }
///     stream.finish();
/// }
/// # fn generate(input: &str) -> Vec<String> { vec![input.to_string()] }
/// ```
///
/// Reporting real progress turns the spinner into a bar:
///
/// ```no_run
/// use foundation_repl::Repl;
///
/// let repl = Repl::new();
/// let work = repl.animation("downloading");
/// for step in 0..=10 {
///     work.set_progress(Some(f64::from(step) / 10.0));
/// }
/// work.finish();
/// ```
pub struct Activity {
    state: Arc<Mutex<ActivityState>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    display: SharedDisplay,
}

impl Activity {
    /// Start an indicator that animates until the returned guard is dropped.
    ///
    /// Usually reached through [`Repl::animation`](crate::Repl::animation).
    /// It is public so a caller driving the renderer itself — a browser, where
    /// there is no blocking message loop — can still show one.
    #[must_use]
    pub fn start(display: SharedDisplay, style: ActivityStyle, label: String) -> Self {
        let state = Arc::new(Mutex::new(ActivityState {
            label,
            progress: None,
        }));
        let stop = Arc::new(AtomicBool::new(false));

        let worker = spawn_animation(
            Arc::clone(&display),
            style,
            Arc::clone(&state),
            Arc::clone(&stop),
        );

        Self {
            state,
            stop,
            worker,
            display,
        }
    }

    /// Change the text shown beside the animation.
    ///
    /// The next frame picks it up, so a long-running job can narrate its stages
    /// without redrawing anything itself.
    pub fn set_label(&self, label: impl Into<String>) {
        lock(&self.state).label = label.into();
    }

    /// Switch between a spinner and a progress bar.
    ///
    /// `Some(ratio)` draws a bar filled to `ratio`, clamped to `0.0..=1.0`.
    /// `None` goes back to the spinner.
    pub fn set_progress(&self, progress: Option<f64>) {
        lock(&self.state).progress = progress.map(|ratio| ratio.clamp(0.0, 1.0));
    }

    /// Stream text into the output region above the indicator.
    ///
    /// Text arrives exactly as given — no newline is added, so a token stream
    /// can be pushed a fragment at a time. Rows that fill up are committed to
    /// the terminal's scrollback; the unfinished tail stays live and is flushed
    /// when the activity ends.
    pub fn push(&self, text: impl AsRef<str>) {
        let mut display = lock(&self.display);
        display.stream_push(text.as_ref());
    }

    /// Stream `text` followed by a newline.
    pub fn push_line(&self, text: impl AsRef<str>) {
        let mut display = lock(&self.display);
        display.stream_push(text.as_ref());
        display.stream_push("\n");
    }

    /// Take the indicator down now, rather than at end of scope.
    pub fn finish(self) {
        drop(self);
    }
}

impl fmt::Write for &Activity {
    /// Lets a generation loop `write!` straight into the stream.
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.push(text);
        Ok(())
    }
}

impl fmt::Debug for Activity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = lock(&self.state);
        f.debug_struct("Activity")
            .field("label", &state.label)
            .field("progress", &state.progress)
            .field("running", &!self.stop.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl fmt::Display for Activity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Activity({})", lock(&self.state).label)
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);

        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                tracing::error!("repl: the activity indicator thread panicked");
            }
        }

        // Only tear down once the thread is joined, or a frame still in flight
        // could be drawn after the teardown and strand the indicator on screen.
        lock(&self.display).end_activity();
    }
}

/// Run the animation on its own thread until told to stop.
///
/// WHY: the caller is blocked inside its own work — if the animation were
/// driven from the caller's thread it would only advance when the caller had
/// time to advance it, which is exactly when nothing needs animating.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_animation(
    display: SharedDisplay,
    style: ActivityStyle,
    state: Arc<Mutex<ActivityState>>,
    stop: Arc<AtomicBool>,
) -> Option<JoinHandle<()>> {
    let started = Instant::now();

    let worker = std::thread::Builder::new()
        .name("repl-activity".into())
        .spawn(move || {
            let mut tick = 0_usize;

            while !stop.load(Ordering::Relaxed) {
                {
                    let current = lock(&state);
                    let frame = style
                        .frames
                        .get(tick % style.frames.len().max(1))
                        .map_or("", String::as_str);

                    lock(&display).render_activity(&ActivityView {
                        frame,
                        label: &current.label,
                        elapsed: style.show_elapsed.then(|| started.elapsed()),
                        progress: current.progress,
                    });
                }

                tick = tick.wrapping_add(1);
                std::thread::sleep(style.interval);
            }
        });

    match worker {
        Ok(handle) => Some(handle),
        Err(error) => {
            tracing::warn!(%error, "repl: could not start the activity indicator");
            None
        }
    }
}

/// Draw a single frame; wasm has no threads to animate one with.
#[cfg(target_arch = "wasm32")]
fn spawn_animation(
    display: SharedDisplay,
    style: ActivityStyle,
    state: Arc<Mutex<ActivityState>>,
    _stop: Arc<AtomicBool>,
) -> Option<JoinHandle<()>> {
    let current = lock(&state);
    lock(&display).render_activity(&ActivityView {
        frame: style.frames.first().map_or("", String::as_str),
        label: &current.label,
        elapsed: style.show_elapsed.then(Duration::default),
        progress: current.progress,
    });
    None
}

/// Lock a mutex, taking the value back even if a holder panicked.
///
/// The guarded values are a label, a progress ratio and a renderer. A panic
/// cannot leave any of them in a state that makes the next writer wrong, so the
/// data is worth more than the poison flag.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
