/// nix-based raw ptrace backend.
///
/// Uses `nix::sys::ptrace` for direct `ptrace(2)` calls.
///
/// ## ptrace semantics
///
/// `PTRACE_SEIZE` with `PTRACE_O_TRACESYSGOOD` is used. The execve transition:
/// SEIZE → waitpid (SIGTRAP) → SYSCALL → waitpid (PtraceSyscall, execve entry) →
/// SYSCALL → waitpid (PTRACE_EVENT_EXEC) → SYSCALL → ptrace_loop handles the rest.

use std::collections::HashMap;
use std::sync::Arc;

use nix::libc::{PTRACE_O_TRACECLONE, PTRACE_O_TRACEEXEC, PTRACE_O_TRACEEXIT,
                PTRACE_O_TRACEFORK, PTRACE_O_TRACESYSGOOD, PTRACE_O_TRACEVFORK,
                PTRACE_O_TRACEVFORKDONE};
use nix::sys::ptrace::{self, Options};
use nix::sys::wait::{WaitStatus, waitpid};
use nix::unistd::Pid;

use crate::shared::vfs::error::{VfsError, VfsResult};

use super::syscall_dispatch::{self, SyscallAction, SyscallArgs};
use super::{InterceptorHandle, MountTable, SyscallInterceptor, VirtualFdTable};

/// All ptrace options set atomically via PTRACE_SEIZE.
/// Includes TRACEVFORKDONE so we can properly handle the parent unblock
/// after a vfork exec/exit sequence.
const PTRACE_OPTIONS: Options = Options::from_bits_truncate(
    PTRACE_O_TRACESYSGOOD
    | PTRACE_O_TRACEFORK
    | PTRACE_O_TRACEVFORK
    | PTRACE_O_TRACEVFORKDONE
    | PTRACE_O_TRACECLONE
    | PTRACE_O_TRACEEXEC
    | PTRACE_O_TRACEEXIT,
);

/// Read registers using raw libc::ptrace (PTRACE_GETREGS).
fn get_regs(pid: Pid) -> Option<libc::user_regs_struct> {
    let mut regs: libc::user_regs_struct = unsafe { std::mem::zeroed() };
    let ret = unsafe {
        libc::ptrace(libc::PTRACE_GETREGS, pid.as_raw(), std::ptr::null_mut::<libc::c_void>(),
                     &mut regs as *mut _ as *mut libc::c_void)
    };
    if ret == 0 { Some(regs) } else { None }
}

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

    // Wait for execve ENTRY stop (PtraceSyscall)
    let _exec_entry = waitpid(child, None).map_err(|e| VfsError::Backend {
        message: format!("waitpid for execve entry failed: {e}"),
    })?;

    // Continue through execve. Use PTRACE_CONT (not SYSCALL) to avoid
    // stopping at the execve exit — we want the PTRACE_EVENT_EXEC event.
    ptrace::cont(child, None).map_err(|e| VfsError::Backend {
        message: format!("ptrace cont to complete execve failed: {e}"),
    })?;

    // Wait for PTRACE_EVENT_EXEC
    let exec_status = waitpid(child, None).map_err(|e| VfsError::Backend {
        message: format!("waitpid for exec event failed: {e}"),
    })?;

    // Start tracing the new program
    match exec_status {
        WaitStatus::PtraceEvent(pid, _sig, _event) => {
            ptrace::syscall(pid, None).map_err(|e| VfsError::Backend {
                message: format!("ptrace syscall after exec event failed: {e}"),
            })?;
        }
        WaitStatus::Exited(pid, _code) => {
            return Ok(Pid::from_raw(pid.as_raw()));
        }
        _ => {
            ptrace::cont(child, None).ok();
        }
    }

    Ok(child)
}

fn ptrace_loop(
    child: Pid,
    mount_table: &Arc<MountTable>,
    fd_table: &Arc<VirtualFdTable>,
) -> VfsResult<i32> {
    let mut pending_actions: HashMap<i32, (SyscallAction, i64)> = HashMap::new();

    loop {
        let status = match waitpid(Pid::from_raw(-1), None) {
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
            WaitStatus::Stopped(pid, sig) => {
                if sig == nix::sys::signal::Signal::SIGSTOP {
                    // Group-stop or new child stop — continue with syscall tracing
                    ptrace::syscall(pid, None).ok();
                } else {
                    // Real signal — deliver it and continue with tracing
                    ptrace::syscall(pid, Some(sig)).ok();
                }
            }
            WaitStatus::PtraceSyscall(pid) => {
                // Syscall stop — dispatch using raw getregs
                if let Some(regs) = get_regs(pid) {
                    let is_exit = pending_actions.contains_key(&pid.as_raw());

                    if is_exit {
                        if let Some((action, _syscall_nr)) = pending_actions.remove(&pid.as_raw()) {
                            match action {
                                SyscallAction::Passthrough => {}
                                SyscallAction::Skip { return_value } => {
                                    let mut r = regs;
                                    r.rax = return_value as u64;
                                    ptrace::setregs(pid, r).ok();
                                }
                                SyscallAction::SkipError { errno } => {
                                    let mut r = regs;
                                    r.rax = (-errno) as u64;
                                    ptrace::setregs(pid, r).ok();
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
                } else {
                    // Process gone — continue
                    ptrace::syscall(pid, None).ok();
                }
            }
            WaitStatus::PtraceEvent(pid, _sig, event) => {
                match event {
                    libc::PTRACE_EVENT_STOP => {
                        // Group-stop or initial SEIZE stop. On newer kernels
                        // (7.0+), this arrives during vfork when the parent
                        // is blocked. Detaching lets the process run free;
                        // the tracer continues via waitpid(-1) on remaining
                        // traced children.
                        ptrace::detach(pid, None).ok();
                    }
                    libc::PTRACE_EVENT_VFORK_DONE => {
                        // The child has completed exec/exit and the parent is
                        // unblocked. Resume the parent with syscall tracing.
                        ptrace::syscall(pid, None).ok();
                    }
                    libc::PTRACE_EVENT_FORK
                    | libc::PTRACE_EVENT_VFORK
                    | libc::PTRACE_EVENT_CLONE => {
                        if let Ok(new_pid_raw) = ptrace::getevent(pid) {
                            let new_pid = Pid::from_raw(new_pid_raw as i32);
                            fd_table.clone_for_child(pid.as_raw() as u32, new_pid.as_raw() as u32);
                            ptrace::syscall(new_pid, None).ok();
                        }
                        ptrace::syscall(pid, None).ok();
                    }
                    libc::PTRACE_EVENT_EXEC => {
                        fd_table.close_cloexec(pid.as_raw() as u32);
                        ptrace::syscall(pid, None).ok();
                    }
                    libc::PTRACE_EVENT_EXIT => {
                        ptrace::syscall(pid, None).ok();
                    }
                    _ => {
                        // Unknown ptrace event. May include a child pid — continue it if so.
                        if let Ok(new_pid_raw) = ptrace::getevent(pid) {
                            let new_pid = Pid::from_raw(new_pid_raw as i32);
                            fd_table.clone_for_child(pid.as_raw() as u32, new_pid.as_raw() as u32);
                            ptrace::syscall(new_pid, None).ok();
                        }
                        ptrace::syscall(pid, None).ok();
                    }
                }
            }
            _ => {}
        }
    }
}
