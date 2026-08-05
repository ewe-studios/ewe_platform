//! Platform process control: liveness checks and graceful/forceful termination.
//!
//! WHY: The supervisor's two-phase kill (SIGTERM then SIGKILL) and its liveness
//! polling are inherently OS-specific; isolating them keeps the supervisor code
//! platform-agnostic.
//!
//! WHAT: [`process_alive`], [`terminate`] (graceful), and [`kill`] (forceful),
//! each taking a raw pid.
//!
//! HOW: On unix, children are spawned into their own process group (pgid == pid)
//! so signals reach the whole tree via `killpg`; liveness uses `kill(pid, 0)`.
//! On Windows there are no POSIX process groups, so the single process is
//! terminated via `TerminateProcess` and liveness read via `GetExitCodeProcess`.

/// Return `true` if a process with this pid is still alive.
///
/// # Panics
/// Never panics.
#[cfg(unix)]
#[must_use]
pub fn process_alive(pid: u32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    // Signal 0 performs error checking without delivering a signal: Ok => the
    // process (or a zombie of it) exists; ESRCH => gone.
    kill(Pid::from_raw(pid as i32), None).is_ok()
}

/// Send SIGTERM to the daemon's process group (graceful stop).
///
/// # Errors
/// Returns a message if the signalling syscall fails.
///
/// # Panics
/// Never panics.
#[cfg(unix)]
pub fn terminate(pid: u32) -> Result<(), String> {
    signal_group(pid, nix::sys::signal::Signal::SIGTERM)
}

/// Send SIGKILL to the daemon's process group (forceful stop).
///
/// # Errors
/// Returns a message if the signalling syscall fails.
///
/// # Panics
/// Never panics.
#[cfg(unix)]
pub fn kill(pid: u32) -> Result<(), String> {
    signal_group(pid, nix::sys::signal::Signal::SIGKILL)
}

#[cfg(unix)]
fn signal_group(pid: u32, signal: nix::sys::signal::Signal) -> Result<(), String> {
    use nix::sys::signal::killpg;
    use nix::unistd::Pid;
    // The child was spawned with `process_group(0)`, so its pgid equals its pid.
    killpg(Pid::from_raw(pid as i32), signal).map_err(|e| e.to_string())
}

#[cfg(windows)]
#[must_use]
pub fn process_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: OpenProcess returns null on failure, which we check before use;
    // the handle is closed on every path.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code: u32 = 0;
        let ok = GetExitCodeProcess(handle, &mut code);
        CloseHandle(handle);
        ok != 0 && code == STILL_ACTIVE as u32
    }
}

#[cfg(windows)]
pub fn terminate(pid: u32) -> Result<(), String> {
    // Windows has no SIGTERM; a graceful request degrades to TerminateProcess.
    kill(pid)
}

#[cfg(windows)]
pub fn kill(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    // SAFETY: null handle is checked; the handle is always closed.
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if handle.is_null() {
            return Err(format!("OpenProcess failed for pid {pid}"));
        }
        let ok = TerminateProcess(handle, 1);
        CloseHandle(handle);
        if ok == 0 {
            return Err(format!("TerminateProcess failed for pid {pid}"));
        }
        Ok(())
    }
}
