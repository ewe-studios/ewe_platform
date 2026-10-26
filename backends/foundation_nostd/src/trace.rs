//! A minimal no_std `tracing` subscriber.
//!
//! WHY: `foundation_nostd` has no `std`, so it cannot use `tracing-subscriber`'s
//! `fmt` layer (which needs `std::io`). This subscriber records the same
//! `tracing::*!` events through a sink the host injects.
//!
//! WHAT: [`NoStdSubscriber`] implements [`tracing::Subscriber`]. [`set_sink`]
//! installs a `fn(&str)` that receives one formatted line per event.
//! [`try_init`] installs the subscriber as the process-wide default, if one is
//! not already set.
//!
//! The host is `foundation_wasm` (its `host_console_log` `abi` import) or a
//! native driver. The sink is a plain function pointer: no `std::io` and no
//! alloc-heavy closure is needed. When no sink is set, every event is dropped.

extern crate alloc;

use alloc::string::String;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicPtr, Ordering};

use tracing::field::{Field, Visit};
use tracing::span::Id;
use tracing::{Event, Level, Metadata, Subscriber};

/// One line of formatted trace output.
type Sink = fn(&str);

/// The sink the host sets. `None` (the default) drops every event.
static SINK: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

/// Install the sink the subscriber forwards to.
///
/// Call once, before [`try_init`]. The sink receives one line per enabled
/// event. This is a function pointer so the host can pass a thin `fn(&str)`
/// that wraps its own output channel (for example `host_console_log`).
pub fn set_sink(sink: Sink) {
    SINK.store(sink as *mut (), Ordering::Relaxed);
}

fn sink() -> Option<Sink> {
    let ptr = SINK.load(Ordering::Relaxed);
    if ptr.is_null() {
        None
    } else {
        // SAFETY: `set_sink` stored a `Sink`; a thin function pointer cast to a
        // `*mut ()` and back is lossless.
        Some(unsafe { core::mem::transmute::<*mut (), Sink>(ptr) })
    }
}

/// Writes one event's fields into `out` as `key=value`, separated by spaces.
struct FieldFormatter<'a> {
    out: &'a mut String,
    first: bool,
}

impl Visit for FieldFormatter<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if !self.first {
            let _ = write!(self.out, " ");
        }
        self.first = false;
        let _ = write!(self.out, "{}={:?}", field.name(), value);
    }
    // `record_str`, `record_i64`, `record_u64`, `record_bool`, and the other
    // typed methods default to forwarding into `record_debug`. `record_error`
    // is `#[cfg(feature = "std")]` and absent here.
}

/// A subscriber that formats each event into one line and forwards it to the
/// sink installed by [`set_sink`].
#[derive(Debug, Clone, Copy)]
pub struct NoStdSubscriber {
    max_level: Level,
}

impl NoStdSubscriber {
    /// A subscriber that enables events at or below `max_level`.
    pub const fn new(max_level: Level) -> Self {
        Self { max_level }
    }
}

impl Subscriber for NoStdSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= &self.max_level
    }

    fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
        Some(self.max_level.into())
    }

    fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> Id {
        // Spans are not stored; a shared non-zero id is enough because
        // `enter`/`exit` are no-ops.
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let Some(sink) = sink() else {
            return;
        };
        let meta = event.metadata();
        let mut line = String::new();
        let _ = write!(line, "{} {}: ", meta.level(), meta.target());
        let mut formatter = FieldFormatter {
            out: &mut line,
            first: true,
        };
        event.record(&mut formatter);
        sink(&line);
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

/// Install [`NoStdSubscriber`] with the given max level as the global default.
///
/// Returns `Err` if a subscriber is already set. A library should usually let
/// the host call this, but a no_std binary has no other entry point.
pub fn try_init(max_level: Level) -> Result<(), tracing::subscriber::SetGlobalDefaultError> {
    tracing::subscriber::set_global_default(NoStdSubscriber::new(max_level))
}
