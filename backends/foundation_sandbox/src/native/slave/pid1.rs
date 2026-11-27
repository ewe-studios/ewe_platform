//! The duties of process identifier 1, and the end of the machine.

use std::io;
use std::path::Path;
use std::sync::Arc;

use crate::shared::host::SlaveHost;

use super::sys;

/// The mode of a directory that this file makes.
const DIR_MODE: u32 = 0o755;

/// Whether this process runs in a container.
///
/// Docker writes `/.dockerenv` and Podman writes `/run/.containerenv`. A
/// container runtime owns the mount namespace and the loopback interface, and a
/// container's process identifier (PID) 1 has no right to change them. The
/// slave then skips the virtual machine boot duties and ends with an exit
/// instead of a power off (feature 10).
#[must_use]
pub fn in_container() -> bool {
    Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists()
}

/// Mounts the kernel file systems, sets the loopback interface up, opens the
/// `SIGCHLD` descriptor and starts the reaper.
///
/// A container runtime already did these steps, so a container's PID 1 skips
/// them: the mount calls return `EPERM` there, not a useful error.
///
/// # Errors
/// The operating system error of the first mount or of the signal set-up.
pub fn boot_duties(host: &SlaveHost) -> io::Result<()> {
    if in_container() {
        return Ok(());
    }
    for (source, target, fstype) in [
        ("proc", "/proc", "proc"),
        ("sysfs", "/sys", "sysfs"),
        ("devtmpfs", "/dev", "devtmpfs"),
        ("devpts", "/dev/pts", "devpts"),
        ("tmpfs", "/run", "tmpfs"),
    ] {
        let target = Path::new(target);
        if let Err(error) = host.mounts.mkdir_p(target, DIR_MODE) {
            // The initramfs of feature 03 already has the mount points. A
            // directory that exists needs no second call.
            if error.raw_os_error() != Some(libc::EEXIST) {
                return Err(error);
            }
        }
        match host.mounts.mount(source, target, fstype, 0, "") {
            Ok(()) => {}
            // A file system that is mounted already is a finished step.
            Err(error) if error.raw_os_error() == Some(libc::EBUSY) => {}
            Err(error) => return Err(error),
        }
    }
    sys::bring_up_loopback()
}

/// Flushes the buffers and powers the machine off.
///
/// # Errors
/// The operating system error of the reboot call. A successful call never
/// returns.
pub fn power_off(host: &SlaveHost) -> io::Result<std::convert::Infallible> {
    host.power.sync();
    host.power.power_off()
}

/// Runs the end of a process identifier 1 slave: the buffers are flushed and
/// the machine is powered off. A failed power off writes the error and waits
/// for ever, so the control plane forces the stop after its grace time.
///
/// In a container the runtime owns the machine, so the slave ends the container
/// with exit code 0 instead (feature 10).
pub fn finish_and_power_off(host: &Arc<SlaveHost>) -> ! {
    if in_container() {
        host.power.sync();
        std::process::exit(0);
    }
    match power_off(host) {
        Ok(never) => match never {},
        Err(error) => {
            let text = format!("agent-slave: the power off failed: {error}\n");
            // The console is descriptor 2. A failed write has no other report.
            let _ = sys::write(2, text.as_bytes());
            sys::pause_forever()
        }
    }
}
