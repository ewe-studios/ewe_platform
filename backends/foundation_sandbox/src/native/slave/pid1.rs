//! The duties of process identifier 1, and the end of the machine.

use std::io;
use std::path::Path;
use std::sync::Arc;

use crate::shared::host::SlaveHost;

use super::sys;

/// The mode of a directory that this file makes.
const DIR_MODE: u32 = 0o755;

/// Mounts the kernel file systems, sets the loopback interface up, opens the
/// `SIGCHLD` descriptor and starts the reaper.
///
/// # Errors
/// The operating system error of the first mount or of the signal set-up.
pub fn boot_duties(host: &SlaveHost) -> io::Result<()> {
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
pub fn finish_and_power_off(host: &Arc<SlaveHost>) -> ! {
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
