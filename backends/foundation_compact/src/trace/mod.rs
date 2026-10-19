//! Test tracing init — native via `tracing-subscriber`, wasm via no-op stub.
//!
//! WHY: Every `#[valtron_test]` needs a tracing subscriber so `tracing::debug!()`
//! diagnostics land on stderr. On native, `tracing-subscriber::fmt` does the work.
//! On wasm32-unknown-unknown, `tracing-subscriber` is unavailable (no threads, no
//! `std::io::stderr`), so we provide a no-op — the caller or a wasm-specific test
//! harness installs a `tracing-wasm` subscriber themselves.
//!
//! WHAT: [`TracingTestConfig`] exposes every knob the `fmt` builder has
//! (`env_filter`, `targets`, `thread_ids`, …), plus a convenience
//! [`try_init_tracing`] that uses defaults.

/// Configuration for [`try_init_tracing_with`] — every knob the
/// `tracing_subscriber::fmt` builder exposes, gathered into one struct so callers
/// and the `#[valtron_test]` macro can tune tracing without touching the RUST_LOG
/// env var.
///
/// Defaults: libtest-captured output, no targets/file/line/ansi, thread ids on,
/// filter from `RUST_LOG` (fallback `"info"`).
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
    /// If some other subscriber was installed first, this leaves it alone.
    pub fn try_init_tracing_with(config: TracingTestConfig) {
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

// ── Wasm ────────────────────────────────────────────────────────────────────

#[cfg(all(target_family = "wasm", any(target_os = "unknown", target_os = "none")))]
mod imp {
    use super::TracingTestConfig;

    /// No-op on wasm — `tracing-subscriber` is native-only. Callers install a
    /// wasm-compatible subscriber (e.g. `tracing-wasm`) in their test harness.
    pub fn try_init_tracing_with(_config: TracingTestConfig) {
        // tracing-wasm or similar must be installed by the caller / test harness.
    }

    /// No-op on wasm.
    pub fn try_init_tracing() {}
}

pub use imp::*;
