/// nix-based raw ptrace backend.
///
/// Uses `nix::sys::ptrace` for direct `ptrace(2)` calls:
/// - `PTRACE_TRACEME` on the child
/// - `PTRACE_SYSCALL` to trap syscall entry/exit
/// - `PTRACE_GETREGS` to read syscall number + arguments
/// - `PTRACE_SETREGS` to modify return values
///
/// No external framework dependency — just the `nix` crate.

use std::sync::Arc;

use nix::sys::ptrace;
use nix::sys::ptrace::Options as PtraceOptions;
use nix::sys::wait::waitpid;
use nix::unistd::Pid;

use crate::shared::vfs::error::{VfsError, VfsResult};

use super::syscall_dispatch::{self, SyscallAction, SyscallArgs};
use super::{InterceptorHandle, MountTable, SyscallInterceptor, VirtualFdTable};

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

/// Fork and exec a child process under ptrace.
fn spawn_child(command: &str, args: &[&str]) -> VfsResult<Pid> {
    // Fork
    let child = match unsafe { nix::unistd::fork() } {
        Ok(nix::unistd::ForkResult::Child) => {
            // Child: enable tracing and exec
            ptrace::traceme().map_err(|e| VfsError::Backend {
                message: format!("ptrace traceme failed: {e}"),
            })?;

            // Set ptrace options for fork/clone tracing
            // This is done by the parent via PTRACE_SETOPTIONS after the first stop
            // but traceme() starts tracing immediately

            // Build argument list (command + args)
            let command_c = std::ffi::CString::new(command).unwrap();
            let args_c: Vec<std::ffi::CString> = args.iter()
                .map(|s| std::ffi::CString::new(*s).unwrap())
                .collect();

            // Exec
            nix::unistd::execvp(&command_c, &args_c)
                .map_err(|e| VfsError::Backend {
                    message: format!("execvp failed for {command}: {e}"),
                })?;

            // Should never reach here
            std::process::exit(1);
        }
        Ok(nix::unistd::ForkResult::Parent { child, .. }) => child,
        Err(e) => return Err(VfsError::Backend {
            message: format!("fork failed: {e}"),
        }.into()),
    };

    // Parent: wait for the initial SIGTRAP from exec
    let _status = nix::sys::wait::waitpid(child, None).map_err(|e| VfsError::Backend {
        message: format!("waitpid after traceme failed: {e}"),
    })?;

    // Set ptrace options for fork/clone/exec tracing
    let options = PtraceOptions::PTRACE_O_TRACEFORK
        | PtraceOptions::PTRACE_O_TRACECLONE
        | PtraceOptions::PTRACE_O_TRACEVFORK
        | PtraceOptions::PTRACE_O_TRACEEXEC;

    ptrace::setoptions(child, options).map_err(|e| VfsError::Backend {
        message: format!("ptrace setoptions failed: {e}"),
    })?;

    // Continue the child until it hits exec
    ptrace::cont(child, None).map_err(|e| VfsError::Backend {
        message: format!("ptrace cont after traceme failed: {e}"),
    })?;

    // Wait for exec stop
    let _status = nix::sys::wait::waitpid(child, None).map_err(|e| VfsError::Backend {
        message: format!("waitpid for exec stop failed: {e}"),
    })?;

    Ok(child)
}

/// Main ptrace loop — intercept syscalls and dispatch to VFS.
fn ptrace_loop(
    child: Pid,
    mount_table: &Arc<MountTable>,
    fd_table: &Arc<VirtualFdTable>,
) -> VfsResult<i32> {
    let mut tracee_pid = child;
    let mut pending_actions: std::collections::HashMap<i32, (SyscallAction, i64)> = std::collections::HashMap::new();

    loop {
        let status = match waitpid(tracee_pid, None) {
            Ok(s) => s,
            Err(nix::errno::Errno::ECHILD) => return Ok(0),
            Err(e) => {
                return Err(VfsError::Backend {
                    message: format!("waitpid failed: {e}"),
                }.into());
            }
        };

        use nix::sys::wait::WaitStatus;
        match status {
            WaitStatus::Exited(pid, exit_code) => {
                fd_table.close_all(pid.as_raw() as u32);
                pending_actions.remove(&pid.as_raw());
                return Ok(exit_code);
            }
            WaitStatus::Signaled(pid, sig, _core_dump) => {
                fd_table.close_all(pid.as_raw() as u32);
                pending_actions.remove(&pid.as_raw());
                return Ok(128 + sig as i32);
            }
            WaitStatus::Stopped(pid, _sig) => {
                handle_ptrace_stop(pid, mount_table, fd_table, &mut pending_actions, None);
                tracee_pid = pid;
            }
            WaitStatus::PtraceEvent(pid, _sig, event) => {
                handle_ptrace_stop(pid, mount_table, fd_table, &mut pending_actions, Some(event));
                tracee_pid = pid;
            }
            _ => { tracee_pid = child; }
        }
    }
}

/// Handle a single ptrace stop for a traced process.
fn handle_ptrace_stop(
    pid: Pid,
    mount_table: &Arc<MountTable>,
    fd_table: &Arc<VirtualFdTable>,
    pending_actions: &mut std::collections::HashMap<i32, (SyscallAction, i64)>,
    event: Option<i32>,
) {
    // Handle fork/clone/exec events from PtraceEvent
    if let Some(event) = event {
        let event_u = event as u32;
        if event_u == libc::PTRACE_EVENT_FORK as u32
            || event_u == libc::PTRACE_EVENT_VFORK as u32
            || event_u == libc::PTRACE_EVENT_CLONE as u32
        {
            if let Ok(new_pid) = ptrace::getevent(pid) {
                let new_pid = new_pid as i32;
                fd_table.clone_for_child(pid.as_raw() as u32, new_pid as u32);
            }
        }
        // PTRACE_EVENT_EXEC: the new process image is loaded, continue normally
    }

    // Read registers to get syscall number + args
    let regs = match ptrace::getregs(pid) {
        Ok(r) => r,
        Err(_) => {
            // Can't read registers — pass through
            ptrace::cont(pid, None).ok();
            return;
        }
    };

    let is_exit = pending_actions.contains_key(&pid.as_raw());

    if is_exit {
        // Syscall exit — apply the action from entry
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
        // Syscall entry — dispatch
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