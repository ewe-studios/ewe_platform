//! Bootstrap logging — writes step progress to `$PWD/.testbed/<vm-name>/bootstrap.log`.
//!
//! Each line is timestamped. Both stdout and the log file receive the same
//! output, so CLI users see progress in real time while the log captures
//! the full record for later inspection.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

/// Thread-safe bootstrap logger.
pub struct BootstrapLogger {
    vm_name: String,
    log_file: Mutex<File>,
    start: Instant,
}

impl BootstrapLogger {
    pub fn new(vm_name: &str) -> std::io::Result<Self> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let log_dir = cwd.join(".testbed").join(vm_name);
        fs::create_dir_all(&log_dir)?;
        let log_path = log_dir.join("bootstrap.log");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        Ok(Self {
            vm_name: vm_name.to_string(),
            log_file: Mutex::new(file),
            start: Instant::now(),
        })
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
        let elapsed = self.start.elapsed();
        let _ = self.log_line(&format!(
            "[{:.1}s] [bootstrap] {} → {}...",
            elapsed.as_secs_f64(),
            self.vm_name,
            label,
        ));
    }

    pub fn step_done(&self, label: &str, duration: std::time::Duration) {
        let total = self.start.elapsed();
        let _ = self.log_line(&format!(
            "[{:.1}s] [bootstrap] {} ✓ {} ({:?})",
            total.as_secs_f64(),
            self.vm_name,
            label,
            duration,
        ));
    }

    pub fn message(&self, msg: &str) {
        let elapsed = self.start.elapsed();
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
        PathBuf::from(".testbed").join(&self.vm_name).join("bootstrap.log")
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.start.elapsed()
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
pub fn step<F>(logger: &BootstrapLogger, label: &str, f: F) -> crate::vms::config::Result<()>
where
    F: FnOnce() -> crate::vms::config::Result<()>,
{
    logger.step_start(label);
    let start = Instant::now();
    let result = f();
    let elapsed = start.elapsed();
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

