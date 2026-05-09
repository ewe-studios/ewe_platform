//! Debloat CLI handler — removes pre-installed bloatware from Windows VMs.
//!
//! Separate from bootstrap so users can run it only when needed.

use clap::ArgMatches;
use std::time::{Duration, Instant};

use crate::bootstrap;
use crate::config::{get_profile, GuestOs};
use crate::ssh;
use crate::state;
use crate::winrm::WinRM;
use crate::winrm::elevated::ProgressCallback;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

pub fn cmd_debloat(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let profile = get_profile(name).map(|p| p.clone())
        .map_err(|e| format!("Unknown profile '{name}': {e}"))?;

    if profile.os != GuestOs::Windows {
        return Err(format!("Debloat is only supported on Windows VMs").into());
    }

    // Ensure VM is running
    if !state::is_running(&profile.name) {
        return Err(format!("VM '{}' is not running. Start it first with `testbed start`", profile.name).into());
    }

    let vm_state = state::load(&profile.name)
        .expect("is_running returned true but load failed");

    let winrm_port = vm_state.winrm_port;

    eprintln!("  Connecting to VM via WinRM (port {winrm_port})...");
    let start = Instant::now();
    let timeout = Duration::from_secs(120);
    let winrm = loop {
        if start.elapsed() > timeout {
            return Err("Timed out waiting for WinRM".into());
        }
        let w = WinRM::new("127.0.0.1", winrm_port, &profile.user, &profile.pass);
        if w.ping() {
            break w;
        }
        std::thread::sleep(Duration::from_secs(2));
    };
    eprintln!("  WinRM connected");

    let progress: ProgressCallback<'_> = Some(&|msg: &str| {
        eprintln!("  {msg}");
    });

    eprintln!("  Running debloat (this may take several minutes)...");
    bootstrap::windows::debloat_windows(&winrm)
        .map_err(|e| format!("Debloat failed: {e}"))?;
    eprintln!("  ✓ Debloat complete");

    Ok(())
}
