//! Bootstrap logging — writes step progress to `$PWD/.testbed/<vm-name>/bootstrap.log`.
//!
//! Each line is timestamped. Both stdout and the log file receive the same
//! output, so CLI users see progress in real time while the log captures
//! the full record for later inspection.

use foundation_compact::Instant;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use foundation_context::{Clock, Ctx};

/// Thread-safe bootstrap logger.
pub struct BootstrapLogger {
    vm_name: String,
    log_file: Mutex<File>,
    /// The host's context. Every step duration and log timestamp reads its
    /// clock, so a test can control both (AGENTS rule 11).
    ctx: Ctx,
    /// The instant the logger was built, on the clock of [`Self::ctx`].
    start: Instant,
}

impl BootstrapLogger {
    /// Create a bootstrap logger for a VM.
    ///
    /// # Errors
    /// Returns an error if the log directory or file cannot be created.
    pub fn new(vm_name: &str, ctx: Ctx) -> std::io::Result<Self> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let log_dir = cwd.join(".testbed").join(vm_name);
        fs::create_dir_all(&log_dir)?;
        let log_path = log_dir.join("bootstrap.log");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let start = ctx.clock().now();
        Ok(Self {
            vm_name: vm_name.to_string(),
            log_file: Mutex::new(file),
            ctx,
            start,
        })
    }

    /// The injected clock that this logger and its callers read.
    #[must_use]
    pub fn clock(&self) -> Arc<dyn Clock> {
        self.ctx.clock()
    }

    /// The injected context, for a client built from this logger.
    #[must_use]
    pub fn ctx(&self) -> &Ctx {
        &self.ctx
    }

    /// The current monotonic instant on the clock of [`Self::ctx`].
    fn now(&self) -> Instant {
        self.ctx.clock().now()
    }

    /// The time since the logger was built, on the clock of [`Self::ctx`].
    fn since_start(&self) -> std::time::Duration {
        self.now().duration_since(self.start)
    }

    fn log_line(&self, line: &str) -> std::io::Result<()> {
        println!("{line}");
        if let Ok(mut f) = self.log_file.lock() {
            writeln!(f, "{line}")?;
            f.flush()?;
        }
        Ok(())
    }

    pub fn step_start(&self, label: &str) {
        let elapsed = self.since_start();
        let _ = self.log_line(&format!(
            "[{:.1}s] [bootstrap] {} → {}...",
            elapsed.as_secs_f64(),
            self.vm_name,
            label,
        ));
    }

    pub fn step_done(&self, label: &str, duration: std::time::Duration) {
        let total = self.since_start();
        let _ = self.log_line(&format!(
            "[{:.1}s] [bootstrap] {} ✓ {} ({:?})",
            total.as_secs_f64(),
            self.vm_name,
            label,
            duration,
        ));
    }

    pub fn message(&self, msg: &str) {
        let elapsed = self.since_start();
        let _ = self.log_line(&format!(
            "[{:.1}s] [bootstrap] {} {msg}",
            elapsed.as_secs_f64(),
            self.vm_name,
        ));
    }

    pub fn vm_name(&self) -> &str {
        &self.vm_name
    }

    pub fn log_path(&self) -> PathBuf {
        PathBuf::from(".testbed")
            .join(&self.vm_name)
            .join("bootstrap.log")
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.since_start()
    }

    /// Log intermediate progress during a long-running step.
    pub fn step_progress(&self, label: &str, step_elapsed: std::time::Duration, msg: &str) {
        let total = self.elapsed();
        let _ = self.log_line(&format!(
            "[{:.1}s] [bootstrap] {}   {} ({:.0}s): {msg}",
            total.as_secs_f64(),
            self.vm_name,
            label,
            step_elapsed.as_secs(),
        ));
    }
}

/// Run a named bootstrap step with logging.
///
/// # Errors
/// Returns the error from the bootstrap step.
pub fn step<F>(logger: &BootstrapLogger, label: &str, f: F) -> crate::config::Result<()>
where
    F: FnOnce() -> crate::config::Result<()>,
{
    logger.step_start(label);
    let start = logger.now();
    let result = f();
    let elapsed = logger.now().duration_since(start);
    match &result {
        Ok(()) => logger.step_done(label, elapsed),
        Err(e) => {
            let total = logger.elapsed();
            let line = format!(
                "[{:.1}s] [bootstrap] {} ✗ {} FAILED after {:?}: {e}",
                total.as_secs_f64(),
                logger.vm_name(),
                label,
                elapsed,
            );
            eprintln!("{line}");
            if let Ok(mut f) = logger.log_file.lock() {
                let _ = writeln!(f, "{line}");
                let _ = f.flush();
            }
        }
    }
    result
}
