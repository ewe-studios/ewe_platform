//! Test and entry-point tracing init — native via `tracing-subscriber`, wasm
//! via a `console.log` writer.
//!
//! WHY: Every `#[valtron_test]` needs a tracing subscriber so `tracing::debug!()`
//! diagnostics land on stderr. On native, `tracing-subscriber::fmt` does the work.
//! On wasm32-unknown-unknown there is no stderr, so a `ConsoleWriter` forwards
//! each line to `console.log` (visible in `wrangler tail`). The `wasm-bindgen`
//! backend reaches `console.log` through `js_sys`; the `host-abi` backend
//! reaches it through the `host_console_log` `abi` import.
//!
//! WHY a custom writer instead of `tracing-wasm`: `tracing-wasm` calls the
//! `performance.mark`/`performance.measure` User Timing API from its span
//! `on_enter`/`on_exit` hooks. Workerd does not implement that API, so the
//! first span freezes the isolate (error 1101). A plain `console.log` writer
//! needs only the console API, which workerd does support.
//!
//! WHAT: [`TracingTestConfig`] exposes every knob the `fmt` builder has
//! (`env_filter`, `targets`, `thread_ids`, …) plus a [`TraceWriter`] backend
//! (`Stdout` or `Console`), and a convenience [`try_init_tracing`] that uses
//! defaults.

/// Where a trace event goes.
///
/// - `Stdout` — the standard output (native). On wasm this is a no-op: there is
///   no stdout to write to.
/// - `Console` — `console.log` (wasm, via `js_sys` or the `host_console_log`
///   `abi` import). On native this falls back to the standard output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TraceWriter {
    /// Standard output (native); no-op on wasm.
    #[default]
    Stdout,
    /// `console.log` (wasm); standard output on native.
    Console,
}

/// Configuration for [`try_init_tracing_with`] — every knob the
/// `tracing_subscriber::fmt` builder exposes, gathered into one struct so callers
/// and the `#[valtron_test]` / `#[wasm_logs]` macros can tune tracing without
/// touching the RUST_LOG env var.
///
/// Defaults: libtest-captured output, no targets/file/line/ansi, thread ids on,
/// filter from `RUST_LOG` (fallback `"info"`), and the `Stdout` writer.
#[derive(Debug, Clone)]
pub struct TracingTestConfig {
    /// Override the standard `RUST_LOG` env-var filter. When `None` (default),
    /// `try_init_tracing_with` checks `RUST_LOG`, falling back to `"info"`.
    /// On wasm this is ignored.
    pub env_filter: Option<String>,
    /// Emit module path of each event (`false`).
    pub targets: bool,
    /// Emit thread ids (`true` on native, `false` on wasm).
    pub thread_ids: bool,
    /// Emit thread names (`false`).
    pub thread_names: bool,
    /// Emit source file location (`false`).
    pub files: bool,
    /// Emit line numbers (`false`).
    pub line_numbers: bool,
    /// ANSI escape codes (`false` — cleaner in log files).
    pub ansi: bool,
    /// Where to send each event ([`TraceWriter::Stdout`] or
    /// [`TraceWriter::Console`]).
    pub writer: TraceWriter,
}

impl Default for TracingTestConfig {
    fn default() -> Self {
        Self {
            env_filter: None,
            targets: false,
            thread_ids: true,
            thread_names: false,
            files: false,
            line_numbers: false,
            ansi: false,
            writer: TraceWriter::Stdout,
        }
    }
}

// ── Native ──────────────────────────────────────────────────────────────────

#[cfg(not(all(target_family = "wasm", any(target_os = "unknown", target_os = "none"))))]
mod imp {
    use super::TracingTestConfig;
    use std::sync::OnceLock;
    use tracing_subscriber::{fmt, layer::SubscriberExt, reload, util::SubscriberInitExt, EnvFilter, Registry};

    /// Handle to the installed subscriber's filter, so each call can set its own.
    static FILTER: OnceLock<Option<reload::Handle<EnvFilter, Registry>>> = OnceLock::new();

    fn filter_for(config: &TracingTestConfig) -> EnvFilter {
        match config.env_filter {
            Some(ref s) => EnvFilter::new(s.clone()),
            None => EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        }
    }

    /// Initialise a tracing subscriber for test diagnostics with the given
    /// [`TracingTestConfig`].
    ///
    /// The first call installs one process-wide subscriber. It writes through
    /// libtest's output capture (`with_test_writer`), so a passing test's logs
    /// stay hidden and a failing test's logs are printed with its failure. Threads
    /// a test spawns, such as valtron's pool workers, inherit that capture.
    ///
    /// Every call, the first included, sets the filter to `config.env_filter`, or
    /// to `RUST_LOG` (fallback `"info"`) when that is `None`. The filter is
    /// process-wide: tests running at the same time share whichever was set last.
    /// The formatting knobs (`targets`, `thread_ids`, …) come from the first call.
    ///
    /// The `writer` field is ignored on native: both [`TraceWriter::Stdout`] and
    /// [`TraceWriter::Console`] write through the same standard output.
    ///
    /// If some other subscriber was installed first, this leaves it alone.
    pub fn try_init_tracing_with(config: TracingTestConfig) {
        let _ = config.writer; // Stdout and Console are the same sink on native.
        let handle = FILTER.get_or_init(|| {
            let (filter, handle) = reload::Layer::new(filter_for(&config));
            let installed = tracing_subscriber::registry()
                .with(filter)
                .with(
                    fmt::layer()
                        .with_target(config.targets)
                        .with_thread_ids(config.thread_ids)
                        .with_thread_names(config.thread_names)
                        .with_file(config.files)
                        .with_line_number(config.line_numbers)
                        .with_ansi(config.ansi)
                        .with_test_writer(),
                )
                .try_init()
                .is_ok();
            installed.then_some(handle)
        });
        if let Some(handle) = handle {
            // Only fails if the subscriber was dropped, which a global one never is.
            let _ = handle.reload(filter_for(&config));
        }
    }

    /// Convenience — [`try_init_tracing_with`] with [`TracingTestConfig::default`].
    pub fn try_init_tracing() {
        try_init_tracing_with(TracingTestConfig::default());
    }
}

// ── Wasm + wasm-bindgen (console) ───────────────────────────────────────────

#[cfg(all(target_family = "wasm", any(target_os = "unknown", target_os = "none"), feature = "wasm-bindgen"))]
mod imp {
    use super::{TraceWriter, TracingTestConfig};
    use std::io;
    use std::sync::Once;

    use tracing_subscriber::EnvFilter;
    use wasm_bindgen::{JsCast, JsValue};

    /// Forward one line to `console.log` via `js_sys` (no `web_sys`).
    ///
    /// `console.log` is looked up dynamically, so it works in any wasm-bindgen
    /// environment (Cloudflare Workers, browsers) without pulling the large
    /// `web_sys` crate. The `host-abi` backend (no wasm-bindgen) can supply the
    /// same seam through `foundation_wasm`'s JS script injection.
    fn log_to_console(line: &str) {
        let global = js_sys::global();
        let Ok(console) = js_sys::Reflect::get(&global, &JsValue::from_str("console")) else {
            return;
        };
        let Ok(log) = js_sys::Reflect::get(&console, &JsValue::from_str("log")) else {
            return;
        };
        let Some(log) = log.dyn_ref::<js_sys::Function>() else {
            return;
        };
        let args = js_sys::Array::of1(&JsValue::from_str(line));
        let _ = log.apply(&console, &args);
    }

    /// A `std::io::Write` sink that forwards each flushed line to `console.log`.
    ///
    /// Lines are buffered until a newline arrives, then emitted whole, so a single
    /// event is not fragmented across several `console.log` entries.
    #[derive(Default)]
    pub struct ConsoleWriter {
        buf: String,
    }

    impl io::Write for ConsoleWriter {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            self.buf.push_str(&String::from_utf8_lossy(data));
            while let Some(pos) = self.buf.find('\n') {
                let line: String = self.buf.drain(..=pos).collect();
                let line = line.trim_end_matches(['\n', '\r']);
                if !line.is_empty() {
                    log_to_console(line);
                }
            }
            Ok(data.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            if !self.buf.is_empty() {
                let line = std::mem::take(&mut self.buf);
                log_to_console(&line);
            }
            Ok(())
        }
    }

    fn filter_for(config: &TracingTestConfig) -> EnvFilter {
        match config.env_filter {
            Some(ref s) => EnvFilter::new(s.clone()),
            None => EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        }
    }

    /// Initialise a `console.log` tracing subscriber for the given config.
    ///
    /// Only [`TraceWriter::Console`] installs a subscriber; [`TraceWriter::Stdout`]
    /// is a no-op (there is no stdout on wasm). The subscriber is installed once
    /// per process (a `Once`), so repeated calls — one per request, in a worker —
    /// are cheap and idempotent.
    pub fn try_init_tracing_with(config: TracingTestConfig) {
        if config.writer != TraceWriter::Console {
            return;
        }
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_max_level(tracing::Level::TRACE)
                .without_time()
                .with_ansi(false)
                .with_env_filter(filter_for(&config))
                .with_writer(|| ConsoleWriter::default())
                .try_init();
        });
    }

    /// Convenience — [`try_init_tracing_with`] with [`TracingTestConfig::default`].
    ///
    /// The default writer is [`TraceWriter::Stdout`], so this is a no-op on wasm.
    /// Call [`try_init_tracing_with`] with `writer: TraceWriter::Console` instead.
    pub fn try_init_tracing() {
        try_init_tracing_with(TracingTestConfig::default());
    }
}

// ── Wasm without wasm-bindgen (host-abi) ────────────────────────────────────

#[cfg(all(
    target_family = "wasm",
    any(target_os = "unknown", target_os = "none"),
    not(feature = "wasm-bindgen"),
    feature = "host-abi"
))]
mod imp {
    use super::{TraceWriter, TracingTestConfig};
    use std::io;
    use std::sync::Once;

    use tracing_subscriber::EnvFilter;

    // Forward one line to the host `console.log` through the `abi` import.
    //
    // No wasm-bindgen: `host_console_log` is supplied by `foundation_wasm`'s
    // `foundation-wasm.js` runtime (its `web_abi` object), like the clock and
    // entropy imports.
    #[link(wasm_import_module = "abi")]
    unsafe extern "C" {
        fn host_console_log(ptr: u64, len: u64);
    }

    fn log_to_console(line: &str) {
        // SAFETY: `line` is valid UTF-8 in linear memory for the whole call.
        unsafe { host_console_log(line.as_ptr() as u64, line.len() as u64) };
    }

    /// A `std::io::Write` sink that forwards each flushed line to `console.log`.
    #[derive(Default)]
    pub struct ConsoleWriter {
        buf: String,
    }

    impl io::Write for ConsoleWriter {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            self.buf.push_str(&String::from_utf8_lossy(data));
            while let Some(pos) = self.buf.find('\n') {
                let line: String = self.buf.drain(..=pos).collect();
                let line = line.trim_end_matches(['\n', '\r']);
                if !line.is_empty() {
                    log_to_console(line);
                }
            }
            Ok(data.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            if !self.buf.is_empty() {
                let line = std::mem::take(&mut self.buf);
                log_to_console(&line);
            }
            Ok(())
        }
    }

    fn filter_for(config: &TracingTestConfig) -> EnvFilter {
        match config.env_filter {
            Some(ref s) => EnvFilter::new(s.clone()),
            None => EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        }
    }

    /// Initialise a `console.log` tracing subscriber for the given config.
    pub fn try_init_tracing_with(config: TracingTestConfig) {
        if config.writer != TraceWriter::Console {
            return;
        }
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_max_level(tracing::Level::TRACE)
                .without_time()
                .with_ansi(false)
                .with_env_filter(filter_for(&config))
                .with_writer(|| ConsoleWriter::default())
                .try_init();
        });
    }

    /// Convenience — [`try_init_tracing_with`] with [`TracingTestConfig::default`].
    pub fn try_init_tracing() {
        try_init_tracing_with(TracingTestConfig::default());
    }
}

// ── Wasm with neither backend feature ───────────────────────────────────────

// Neither `wasm-bindgen` nor `host-abi` is on, so there is no console to write
// to and no `tracing-subscriber` dependency. Initialising tracing does nothing.
#[cfg(all(
    target_family = "wasm",
    any(target_os = "unknown", target_os = "none"),
    not(feature = "wasm-bindgen"),
    not(feature = "host-abi")
))]
mod imp {
    use super::TracingTestConfig;

    /// No-op: this build has no wasm console backend (`wasm-bindgen` or `host-abi`).
    pub fn try_init_tracing_with(_config: TracingTestConfig) {}

    /// No-op: this build has no wasm console backend (`wasm-bindgen` or `host-abi`).
    pub fn try_init_tracing() {}
}

pub use imp::*;
