/// nix-based raw ptrace backend.
///
/// Uses `nix::sys::ptrace` for direct `ptrace(2)` calls.
///
/// ## ptrace semantics
///
/// The interceptor thread forks the child and runs the whole trace itself.
/// A ptrace request must come from the thread that forked (and attached to)
/// the tracee; a request from another thread fails with `ESRCH`, and the
/// tracee stays stopped. The child calls `PTRACE_TRACEME` and stops itself
/// with `SIGSTOP` before `execve`, so the tracer never races the exec:
///
/// `fork` → child `SIGSTOP` stop → `PTRACE_SETOPTIONS` → `PTRACE_SYSCALL` →
/// `PTRACE_EVENT_EXEC` → the syscall loop handles the rest.
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::Arc;

use nix::libc::{
    PTRACE_O_TRACECLONE, PTRACE_O_TRACEEXEC, PTRACE_O_TRACEEXIT, PTRACE_O_TRACEFORK,
    PTRACE_O_TRACESYSGOOD, PTRACE_O_TRACEVFORK, PTRACE_O_TRACEVFORKDONE,
};
use nix::sys::ptrace::{self, Options};
use nix::sys::signal::Signal;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;

use crate::shared::vfs::error::{VfsError, VfsResult};

use super::syscall_dispatch::{self, SyscallAction, SyscallArgs};
use super::{InterceptorHandle, MountTable, SyscallInterceptor, VirtualFdTable};

/// All ptrace options, set with `PTRACE_SETOPTIONS` after the child stops.
/// Includes TRACEVFORKDONE so the parent is resumed after a vfork exec/exit.
const PTRACE_OPTIONS: Options = Options::from_bits_truncate(
    PTRACE_O_TRACESYSGOOD
        | PTRACE_O_TRACEFORK
        | PTRACE_O_TRACEVFORK
        | PTRACE_O_TRACEVFORKDONE
        | PTRACE_O_TRACECLONE
        | PTRACE_O_TRACEEXEC
        | PTRACE_O_TRACEEXIT,
);

/// Wait for any child of this tracer thread, including children created with
/// `CLONE_THREAD`. `__WALL` makes `waitpid` see a clone child's stop.
/// `__WNOTHREAD` keeps each tracer on the child it forked: tests run in
/// parallel, and a plain `waitpid(-1)` would reap another test's tracee.
const WAIT_FLAGS: Option<WaitPidFlag> = Some(WaitPidFlag::__WALL.union(WaitPidFlag::__WNOTHREAD));

/// Read registers using raw `libc::ptrace` (`PTRACE_GETREGS`).
fn get_regs(pid: Pid) -> Option<libc::user_regs_struct> {
    let mut regs: libc::user_regs_struct = unsafe { std::mem::zeroed() };
    let ret = unsafe {
        libc::ptrace(
            libc::PTRACE_GETREGS,
            pid.as_raw(),
            std::ptr::null_mut::<libc::c_void>(),
            &mut regs as *mut _ as *mut libc::c_void,
        )
    };
    if ret == 0 {
        Some(regs)
    } else {
        None
    }
}

/// nix-based raw ptrace interceptor.
pub struct NixInterceptor;

impl NixInterceptor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NixInterceptor {
    fn default() -> Self {
        Self::new()
    }
}

impl SyscallInterceptor for NixInterceptor {
    fn spawn(
        &self,
        mount_table: MountTable,
        command: &str,
        args: &[&str],
    ) -> VfsResult<InterceptorHandle> {
        let mount_table = Arc::new(mount_table);
        let fd_table = Arc::new(VirtualFdTable::new());
        let command = command.to_owned();
        let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();

        // The thread that forks must also run every ptrace request. It sends
        // the child pid back before it starts tracing.
        let (pid_tx, pid_rx) = std::sync::mpsc::channel();
        let join_handle = std::thread::spawn(move || match fork_child(&command, &args) {
            Ok(child) => {
                let _ = pid_tx.send(Ok(child.as_raw() as u32));
                trace_child(child, &mount_table, &fd_table)
            }
            Err(message) => {
                let _ = pid_tx.send(Err(message.clone()));
                Err(VfsError::Backend { message }.into())
            }
        });

        let child_pid = match pid_rx.recv() {
            Ok(Ok(pid)) => pid,
            Ok(Err(message)) => return Err(VfsError::Backend { message }.into()),
            Err(_) => {
                return Err(VfsError::Backend {
                    message: "interceptor thread stopped before the child started".into(),
                }
                .into())
            }
        };

        Ok(InterceptorHandle::new(child_pid, join_handle))
    }
}

/// Fork the child. The child traces itself and stops with `SIGSTOP`.
///
/// All buffers are built before the fork. Between `fork` and `execve` the
/// child calls only async-signal-safe functions, because another thread in
/// the parent may hold the allocator lock.
fn fork_child(command: &str, args: &[String]) -> Result<Pid, String> {
    let command_c =
        CString::new(command).map_err(|e| format!("invalid command {command:?}: {e}"))?;
    let mut argv: Vec<CString> = Vec::with_capacity(args.len() + 1);
    argv.push(command_c.clone());
    for arg in args {
        argv.push(
            CString::new(arg.as_str()).map_err(|e| format!("invalid argument {arg:?}: {e}"))?,
        );
    }
    // argv must include the program name and end with a null pointer.
    let mut argv_ptr: Vec<*const libc::c_char> = argv.iter().map(|arg| arg.as_ptr()).collect();
    argv_ptr.push(std::ptr::null());

    match unsafe { nix::unistd::fork() } {
        Ok(nix::unistd::ForkResult::Child) => unsafe {
            libc::ptrace(libc::PTRACE_TRACEME);
            libc::raise(libc::SIGSTOP);
            // `execvp` replaces the child image on success and never returns.
            libc::execvp(command_c.as_ptr(), argv_ptr.as_ptr());
            // exec failed: exit without running the parent's atexit handlers.
            libc::_exit(1)
        },
        Ok(nix::unistd::ForkResult::Parent { child }) => Ok(child),
        Err(e) => Err(format!("fork failed: {e}")),
    }
}

/// Wait for the child's first stop, then trace it until it exits.
fn trace_child(
    child: Pid,
    mount_table: &Arc<MountTable>,
    fd_table: &Arc<VirtualFdTable>,
) -> VfsResult<i32> {
    // The child stops with SIGSTOP after PTRACE_TRACEME and raise(SIGSTOP).
    let status = waitpid(child, WAIT_FLAGS).map_err(|e| VfsError::Backend {
        message: format!("waitpid for the first child stop failed: {e}"),
    })?;

    match status {
        WaitStatus::Exited(_, code) => return Ok(code),
        WaitStatus::Signaled(_, signal, _) => return Ok(128 + signal as i32),
        _ => {}
    }

    ptrace::setoptions(child, PTRACE_OPTIONS).map_err(|e| VfsError::Backend {
        message: format!("ptrace setoptions failed: {e}"),
    })?;
    ptrace::syscall(child, None).map_err(|e| VfsError::Backend {
        message: format!("ptrace syscall failed: {e}"),
    })?;

    ptrace_loop(child, mount_table, fd_table)
}

fn ptrace_loop(
    child: Pid,
    mount_table: &Arc<MountTable>,
    fd_table: &Arc<VirtualFdTable>,
) -> VfsResult<i32> {
    let mut pending_actions: HashMap<i32, (SyscallAction, i64)> = HashMap::new();

    loop {
        let status = match waitpid(Pid::from_raw(-1), WAIT_FLAGS) {
            Ok(s) => s,
            Err(nix::errno::Errno::ECHILD) => return Ok(0),
            Err(e) => {
                return Err(VfsError::Backend {
                    message: format!("waitpid failed: {e}"),
                }
                .into());
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
                // A new child from TRACEFORK/TRACEVFORK/TRACECLONE stops here
                // the first time with SIGSTOP. Consume that stop and let the
                // child run. Any other signal is delivered to the tracee.
                let deliver = if sig == Signal::SIGSTOP {
                    None
                } else {
                    Some(sig)
                };
                ptrace::syscall(pid, deliver).ok();
            }
            WaitStatus::PtraceSyscall(pid) => {
                // Syscall stop — dispatch using raw getregs.
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
                    // The tracer cannot read the registers, so it cannot
                    // dispatch the syscall. Report it; do not leave the
                    // tracee stopped.
                    return Err(VfsError::Backend {
                        message: format!(
                            "PTRACE_GETREGS failed for {pid}: {}",
                            nix::errno::Errno::last()
                        ),
                    }
                    .into());
                }
            }
            WaitStatus::PtraceEvent(pid, _sig, event) => {
                match event {
                    libc::PTRACE_EVENT_FORK
                    | libc::PTRACE_EVENT_VFORK
                    | libc::PTRACE_EVENT_CLONE => {
                        if let Ok(new_pid_raw) = ptrace::getevent(pid) {
                            let new_pid = Pid::from_raw(new_pid_raw as i32);
                            fd_table.clone_for_child(pid.as_raw() as u32, new_pid.as_raw() as u32);
                            // Do not resume the new child here. Its own first
                            // stop arrives in this loop and is consumed there.
                        }
                        ptrace::syscall(pid, None).ok();
                    }
                    libc::PTRACE_EVENT_VFORK_DONE => {
                        // The child has completed exec/exit and the parent is
                        // unblocked. Resume the parent with syscall tracing.
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
                        ptrace::syscall(pid, None).ok();
                    }
                }
            }
            _ => {}
        }
    }
}
