/// nix-based raw ptrace backend.
///
/// Uses `nix::sys::ptrace` for direct `ptrace(2)` calls:
/// - `PTRACE_SEIZE` on the child (modern alternative to TRACEME)
/// - `PTRACE_SYSCALL` to trap syscall entry/exit
/// - `PTRACE_GETREGS` to read syscall number + arguments
/// - `PTRACE_SETREGS` to modify return values
///
/// No external framework dependency — just the `nix` crate.
///
/// ## ptrace semantics
///
/// `PTRACE_SEIZE` is used instead of `PTRACE_TRACEME` because it allows setting
/// trace options atomically before the child starts executing. With `PTRACE_O_TRACESYSGOOD`,
/// syscall stops are delivered as SIGTRAP | 0x80 (signal 137), so we can distinguish
/// them from plain signal stops by checking `WSTOPSIG(status)`.
///
/// ## execve transition flow
///
/// ```text
/// fork()
///   ├─ child: execvp() → blocked until traced
///   └─ parent: PTRACE_SEIZE(child, OPTIONS) → stops child before execve runs
///              waitpid(child) → initial SIGTRAP stop (from SEIZE)
///              PTRACE_SYSCALL(child) → continue to execve ENTRY
///              waitpid(child) → execve ENTRY syscall stop (SIGTRAP|0x80)
///              PTRACE_SYSCALL(child) → continue through execve
///              waitpid(child) → PTRACE_EVENT_EXEC stop
///              PTRACE_SYSCALL(child) → start tracing new program
///              waitpid(child) → first syscall entry of new program
///              [ptrace_loop takes over from here]
/// ```

use std::collections::HashMap;
use std::sync::Arc;

use nix::libc::{PTRACE_O_TRACECLONE, PTRACE_O_TRACEEXEC, PTRACE_O_TRACEEXIT,
                PTRACE_O_TRACEFORK, PTRACE_O_TRACESYSGOOD, PTRACE_O_TRACEVFORK};
use nix::sys::ptrace::{self, Options};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;

use crate::shared::vfs::error::{VfsError, VfsResult};

use super::syscall_dispatch::{self, SyscallAction, SyscallArgs};
use super::{InterceptorHandle, MountTable, SyscallInterceptor, VirtualFdTable};

/// Signal number for SIGTRAP with the SYSGOOD bit set.
/// When PTRACE_O_TRACESYSGOOD is active, syscall stops deliver signal 137
/// instead of the normal SIGTRAP (5).
const SYSCALL_STOP_SIGNAL: i32 = libc::SIGTRAP | 0x80;

/// All ptrace options set atomically via PTRACE_SEIZE.
const PTRACE_OPTIONS: Options = Options::from_bits_truncate(
    PTRACE_O_TRACESYSGOOD
    | PTRACE_O_TRACEFORK
    | PTRACE_O_TRACEVFORK
    | PTRACE_O_TRACECLONE
    | PTRACE_O_TRACEEXEC
    | PTRACE_O_TRACEEXIT,
);

/// nix-based raw ptrace interceptor.
pub struct NixInterceptor;

impl NixInterceptor {
    pub fn new() -> Self { Self }
}

impl SyscallInterceptor for NixInterceptor {
    fn spawn(
        &self,
        mount_table: MountTable,
        command: &str,
        args: &[&str],
    ) -> VfsResult<InterceptorHandle> {
        let child = spawn_child(command, args)?;
        let child_pid = child.as_raw() as u32;

        let mount_table = Arc::new(mount_table);
        let fd_table = Arc::new(VirtualFdTable::new());

        let join_handle = std::thread::spawn(move || {
            ptrace_loop(child, &mount_table, &fd_table)
        });

        Ok(InterceptorHandle::new(child_pid, join_handle))
    }
}

/// Fork a child, seize it with ptrace, and exec the target command.
fn spawn_child(command: &str, args: &[&str]) -> VfsResult<Pid> {
    let child = match unsafe { nix::unistd::fork() } {
        Ok(nix::unistd::ForkResult::Child) => {
            let command_c = std::ffi::CString::new(command).unwrap();
            let args_c: Vec<std::ffi::CString> = args.iter()
                .map(|s| std::ffi::CString::new(*s).unwrap())
                .collect();

            nix::unistd::execvp(&command_c, &args_c)
                .map_err(|e| VfsError::Backend {
                    message: format!("execvp failed for {command}: {e}"),
                })?;

            std::process::exit(1);
        }
        Ok(nix::unistd::ForkResult::Parent { child, .. }) => child,
        Err(e) => return Err(VfsError::Backend {
            message: format!("fork failed: {e}"),
        }.into()),
    };

    // PTRACE_SEIZE stops the child immediately with a SIGTRAP.
    // The child is stopped BEFORE execve starts running.
    ptrace::seize(child, PTRACE_OPTIONS).map_err(|e| VfsError::Backend {
        message: format!("ptrace seize failed: {e}"),
    })?;

    // Wait for the initial SIGTRAP stop from PTRACE_SEIZE
    let _ = waitpid(child, None).map_err(|e| VfsError::Backend {
        message: format!("waitpid after seize failed: {e}"),
    })?;

    // Use PTRACE_SYSCALL to continue. This will stop at the execve ENTRY.
    ptrace::syscall(child, None).map_err(|e| VfsError::Backend {
        message: format!("ptrace syscall after seize failed: {e}"),
    })?;

    // Wait for execve ENTRY stop (syscall stop, SIGTRAP|0x80)
    // We just need to pass through this — no dispatch needed for execve itself.
    let exec_entry = waitpid(child, None).map_err(|e| VfsError::Backend {
        message: format!("waitpid for execve entry failed: {e}"),
    })?;

    // Verify it's a syscall stop
    if let WaitStatus::Stopped(pid, sig) = exec_entry {
        if sig as i32 != SYSCALL_STOP_SIGNAL {
            // Not a syscall stop — might be a signal. Just continue.
            ptrace::cont(pid, None).ok();
        }
    }

    // Use PTRACE_SYSCALL to continue through execve.
    // This will trigger PTRACE_EVENT_EXEC when execve completes.
    ptrace::syscall(child, None).map_err(|e| VfsError::Backend {
        message: format!("ptrace syscall to complete execve failed: {e}"),
    })?;

    // Wait for PTRACE_EVENT_EXEC (new program image loaded)
    let exec_status = waitpid(child, None).map_err(|e| VfsError::Backend {
        message: format!("waitpid for exec event failed: {e}"),
    })?;

    // After the exec event, use PTRACE_SYSCALL to start tracing
    // syscalls of the new program.
    let next_pid = match exec_status {
        WaitStatus::PtraceEvent(pid, _sig, _event) => {
            ptrace::syscall(pid, None).map_err(|e| VfsError::Backend {
                message: format!("ptrace syscall after exec event failed: {e}"),
            })?;
            pid
        }
        WaitStatus::Exited(pid, code) => {
            // execvp failed — child exited
            return Ok(Pid::from_raw(pid.as_raw()));
        }
        _ => child,
    };

    // Now use PTRACE_SYSCALL to start tracing syscalls of the new program.
    // DON'T wait for the first syscall entry here — let the ptrace_loop
    // handle it. After PTRACE_SYSCALL, the child will run and stop at
    // the first syscall entry, which ptrace_loop's waitpid(-1) will pick up.
    if let WaitStatus::PtraceEvent(pid, _sig, _event) = exec_status {
        ptrace::syscall(pid, None).map_err(|e| VfsError::Backend {
            message: format!("ptrace syscall after exec event failed: {e}"),
        })?;
    } else {
        // If we didn't get a PtraceEvent, the child might have exited
        // (execvp failed). Just continue and let the loop handle it.
        ptrace::syscall(child, None).ok();
    }

    // DON'T consume the first syscall stop here. The ptrace_loop thread
    // will pick it up via its waitpid(-1).
    Ok(child)
}

/// Check if a waitpid status indicates a syscall stop.
fn is_syscall_stop(status: &WaitStatus) -> bool {
    match status {
        WaitStatus::Stopped(_pid, sig) => (*sig as i32) == SYSCALL_STOP_SIGNAL,
        _ => false,
    }
}

/// Main ptrace loop — intercept syscalls and dispatch to VFS.
///
/// Uses `waitpid(-1)` to receive stops from all traced processes
/// (including forked children), following the iii-init PID-1 pattern.
fn ptrace_loop(
    child: Pid,
    mount_table: &Arc<MountTable>,
    fd_table: &Arc<VirtualFdTable>,
) -> VfsResult<i32> {
    let mut pending_actions: HashMap<i32, (SyscallAction, i64)> = HashMap::new();

    loop {
        let status = match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::__WALL)) {
            Ok(s) => s,
            Err(nix::errno::Errno::ECHILD) => return Ok(0),
            Err(e) => {
                return Err(VfsError::Backend {
                    message: format!("waitpid failed: {e}"),
                }.into());
            }
        };

        match status {
            WaitStatus::Exited(pid, exit_code) => {
                fd_table.close_all(pid.as_raw() as u32);
                pending_actions.remove(&pid.as_raw());
                if pid == child {
                    return Ok(exit_code);
                }
            }
            WaitStatus::Signaled(pid, sig, _core_dump) => {
                fd_table.close_all(pid.as_raw() as u32);
                pending_actions.remove(&pid.as_raw());
                if pid == child {
                    return Ok(128 + sig as i32);
                }
            }
            WaitStatus::Stopped(pid, _sig) => {
                // With PTRACE_O_TRACESYSGOOD, syscall stops are SIGTRAP|0x80.
                // Non-syscall stops (signals) are regular signals.
                if is_syscall_stop(&status) {
                    handle_syscall_stop(pid, mount_table, fd_table, &mut pending_actions);
                } else {
                    // Signal stop — deliver the signal and continue
                    ptrace::cont(pid, None).ok();
                }
            }
            WaitStatus::PtraceEvent(pid, _sig, event) => {
                match event {
                    libc::PTRACE_EVENT_FORK
                    | libc::PTRACE_EVENT_VFORK
                    | libc::PTRACE_EVENT_CLONE => {
                        if let Ok(new_pid) = ptrace::getevent(pid) {
                            let new_pid = new_pid as i32;
                            fd_table.clone_for_child(pid.as_raw() as u32, new_pid as u32);
                        }
                        ptrace::syscall(pid, None).ok();
                    }
                    libc::PTRACE_EVENT_EXEC => {
                        ptrace::syscall(pid, None).ok();
                    }
                    libc::PTRACE_EVENT_EXIT => {
                        ptrace::syscall(pid, None).ok();
                    }
                    _ => {
                        ptrace::syscall(pid, None).ok();
                    }
                }
            }
            _ => {}
        }
    }
}

/// Handle a syscall entry/exit stop.
///
/// Uses `pending_actions` to track entry vs exit:
/// - If the pid has a pending action → this is the EXIT stop
/// - Otherwise → this is the ENTRY stop
fn handle_syscall_stop(
    pid: Pid,
    mount_table: &Arc<MountTable>,
    fd_table: &Arc<VirtualFdTable>,
    pending_actions: &mut HashMap<i32, (SyscallAction, i64)>,
) {
    let regs = match ptrace::getregs(pid) {
        Ok(r) => r,
        Err(_) => {
            ptrace::syscall(pid, None).ok();
            return;
        }
    };

    let is_exit = pending_actions.contains_key(&pid.as_raw());

    if is_exit {
        if let Some((action, _syscall_nr)) = pending_actions.remove(&pid.as_raw()) {
            match action {
                SyscallAction::Passthrough => {}
                SyscallAction::Skip { return_value } => {
                    let mut regs = regs;
                    regs.rax = return_value as u64;
                    ptrace::setregs(pid, regs).ok();
                }
                SyscallAction::SkipError { errno } => {
                    let mut regs = regs;
                    regs.rax = (-errno) as u64;
                    ptrace::setregs(pid, regs).ok();
                }
            }
        }
        ptrace::syscall(pid, None).ok();
    } else {
        let syscall_args = SyscallArgs {
            nr: regs.orig_rax as i64,
            arg0: regs.rdi,
            arg1: regs.rsi,
            arg2: regs.rdx,
            arg3: regs.r10,
            arg4: regs.r8,
            arg5: regs.r9,
        };

        let action = syscall_dispatch::on_syscall_entry(
            pid,
            &syscall_args,
            mount_table,
            fd_table,
        );

        pending_actions.insert(pid.as_raw(), (action, syscall_args.nr));
        ptrace::syscall(pid, None).ok();
    }
}
