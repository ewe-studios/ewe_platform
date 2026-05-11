//! Log tailing and error extraction from VMs.
//!
//! Supports dump, follow (live tail), and error extraction modes
//! for both build and run logs.

use crate::config::{GuestOs, Result, VmProfile};
use crate::ssh::VmSession;

/// Log kinds available on the VM.
#[derive(Debug, Clone, Copy)]
pub enum LogKind {
    Build,
    Run,
}

impl std::str::FromStr for LogKind {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "build" => Ok(LogKind::Build),
            "run" => Ok(LogKind::Run),
            _ => Err(format!("unknown log kind: {s}")),
        }
    }
}

/// Log mode.
#[derive(Debug, Clone, Copy)]
pub enum LogMode {
    /// Dump the full log.
    Dump,
    /// Follow (live tail) the log.
    Follow,
    /// Extract error stanzas only.
    Errors,
    /// Show last N lines.
    Tail(u32),
}

/// Options for log retrieval.
#[derive(Debug)]
pub struct LogOpts {
    pub kind: LogKind,
    pub mode: LogMode,
}

/// Get the log file path on the VM.
fn log_path(profile: &VmProfile, kind: LogKind) -> String {
    match (profile.os, kind) {
        (GuestOs::Linux, LogKind::Build) => "/home/vagrant/.testbed-build/build.log".to_string(),
        (GuestOs::Linux, LogKind::Run) => "/home/vagrant/.testbed-run/run.log".to_string(),
        (GuestOs::MacOS, LogKind::Build) => "/Users/vagrant/.testbed-build/build.log".to_string(),
        (GuestOs::MacOS, LogKind::Run) => "/Users/vagrant/.testbed-run/run.log".to_string(),
        (GuestOs::Windows, LogKind::Build) => "C:\\Users\\vagrant\\.testbed-build\\build.log".to_string(),
        (GuestOs::Windows, LogKind::Run) => "C:\\Users\\vagrant\\.testbed-run\\run.log".to_string(),
    }
}

/// Retrieve logs from the VM according to options.
pub fn get_logs(profile: &VmProfile, session: &mut VmSession, opts: &LogOpts) -> Result<String> {
    let path = log_path(profile, opts.kind);

    match opts.mode {
        LogMode::Dump => dump_log(session, &path),
        LogMode::Follow => follow_log(profile, session, &path),
        LogMode::Errors => extract_errors(session, &path),
        LogMode::Tail(n) => tail_log(session, &path, n),
    }
}

/// Dump the full log file.
fn dump_log(session: &mut VmSession, path: &str) -> Result<String> {
    crate::ssh::exec(session, &format!("cat {path}"))
}

/// Follow (live tail) the log file.
///
/// Uses `tail -F` on Linux or `Get-Content -Wait` on Windows.
/// This blocks until the user interrupts.
fn follow_log(profile: &VmProfile, session: &mut VmSession, path: &str) -> Result<String> {
    let cmd = match profile.os {
        GuestOs::Linux | GuestOs::MacOS => format!("tail -F {path}"),
        GuestOs::Windows => format!("Get-Content -Path '{path}' -Wait"),
    };
    crate::ssh::exec(session, &cmd)
}

/// Extract error stanzas from the log.
fn extract_errors(session: &mut VmSession, path: &str) -> Result<String> {
    // Read the log content
    let content = crate::ssh::exec(session, &format!("cat {path}"))?;
    crate::build::logs::extract_errors(&content)
}

/// Show the last N lines of the log.
fn tail_log(session: &mut VmSession, path: &str, n: u32) -> Result<String> {
    crate::ssh::exec(session, &format!("tail -n {n} {path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_kind_from_str() {
        assert!("build".parse::<LogKind>().is_ok());
        assert!("run".parse::<LogKind>().is_ok());
        assert!("invalid".parse::<LogKind>().is_err());
    }

    #[test]
    fn test_log_path_linux() {
        let profile = crate::config::get_profile("linux-build").unwrap().clone();
        assert_eq!(
            log_path(&profile, LogKind::Build),
            "/home/vagrant/.testbed-build/build.log"
        );
    }

    #[test]
    fn test_log_path_windows() {
        let profile = crate::config::get_profile("windows-build").unwrap().clone();
        assert!(log_path(&profile, LogKind::Build).contains("Users"));
        assert!(log_path(&profile, LogKind::Build).contains("build.log"));
    }
}
